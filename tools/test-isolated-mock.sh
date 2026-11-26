#!/usr/bin/env bash
# Runs an end-to-end test with a separate mock process in an isolated environment.
set -euo pipefail

platform_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
mock_dir=$(cd -- "$platform_dir/crates/testing/carosign-mock-provider" && pwd -P)

for tool in cargo rustc python3 unshare; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Не найдено необходимое средство: %s\n' "$tool" >&2
        exit 1
    fi
done

host=$(rustc -vV | sed -n 's/^host: //p')
if [[ "$host" != x86_64-unknown-linux-gnu ]]; then
    printf 'Испытательная seccomp-политика рассчитана на x86_64-unknown-linux-gnu; текущий host: %s\n' "$host" >&2
    exit 1
fi
target_libdir=$(rustc --print target-libdir --target x86_64-unknown-linux-musl)
if [[ ! -d "$target_libdir" ]]; then
    printf 'Нет установленной цели x86_64-unknown-linux-musl для сборки мока.\n' >&2
    exit 1
fi

artifacts=$(mktemp -d "${TMPDIR:-/tmp}/carosign-test-artifacts.XXXXXXXX")
trap 'rm -rf -- "$artifacts"' EXIT

printf 'Сборка статического мок-провайдера без сети…\n'
if ! cargo build --offline --manifest-path "$mock_dir/Cargo.toml" \
    --target x86_64-unknown-linux-musl --message-format=json > "$artifacts/mock.json"; then
    printf 'Не удалось собрать мок: проверьте установленную цель musl и локальные зависимости.\n' >&2
    exit 1
fi
printf 'Сборка тестового помощника без сети…\n'
if ! cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
    -p carosign-linux --lib --no-run --message-format=json > "$artifacts/helper.json"; then
    printf 'Не удалось собрать тестовый помощник carosign-linux.\n' >&2
    exit 1
fi

# Use the executable path from Cargo JSON so custom target directories work.
find_executable() {
    python3 - "$1" "$2" "$3" <<'PY'
import json
import os
import sys

source, name, kind = sys.argv[1:]
matches = []
with open(source, encoding="utf-8") as stream:
    for line in stream:
        record = json.loads(line)
        target = record.get("target", {})
        executable = record.get("executable")
        if (record.get("reason") == "compiler-artifact"
                and target.get("name") == name
                and kind in target.get("kind", [])
                and executable):
            matches.append(executable)
matches = list(dict.fromkeys(matches))
if len(matches) != 1 or not os.path.isfile(matches[0]):
    raise SystemExit(f"Не найден единственный исполняемый артефакт {name}: {matches}")
print(os.path.realpath(matches[0]))
PY
}

mock_binary=$(find_executable "$artifacts/mock.json" carosign-mock-provider bin)
helper_binary=$(find_executable "$artifacts/helper.json" carosign_linux lib)

printf 'Испытание VerificationFlow через отдельный broker и mock…\n'
CAROSIGN_FLOW_TEST_TUNNEL=1 CAROSIGN_FLOW_SEPARATE_CLIENT=1 CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    CAROSIGN_RELAY_TEST_RUNNER="$platform_dir/tools/run-test-relay.sh" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::tunnel_broker_mock_reaches_client_after_confirmed_cleanup \
        --ignored --nocapture

printf 'Положительная проверка через отдельный broker и mock…\n'
CAROSIGN_FLOW_TEST_TUNNEL=1 CAROSIGN_FLOW_SEPARATE_CLIENT=1 CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    CAROSIGN_RELAY_TEST_RUNNER="$platform_dir/tools/run-test-relay.sh" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::tunnel_broker_matched_reaches_client_after_independent_verification \
        --ignored --nocapture

printf 'Проверка PAM-клиента через отдельные broker и mock…\n'
CAROSIGN_FLOW_TEST_TUNNEL=1 CAROSIGN_TEST_PAM_OBSERVER=1 \
    CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    CAROSIGN_RELAY_TEST_RUNNER="$platform_dir/tools/run-test-relay.sh" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::tunnel_broker_mock_reaches_client_after_confirmed_cleanup \
        --ignored --nocapture

printf 'Испытание проверки mock через отдельный испытательный broker…\n'
CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    CAROSIGN_RELAY_TEST_RUNNER="$platform_dir/tools/run-test-relay.sh" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosign-broker --lib -- --exact \
        relay_pump::tests::separate_broker_initializes_real_mock \
        --ignored --nocapture

printf 'Испытание передачи синтетического совпадения через broker…\n'
CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" CAROSIGN_RELAY_MATCHED_TEST=1 \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosign-broker --lib -- --exact \
        relay_pump::tests::separate_broker_initializes_real_mock \
        --ignored --nocapture

printf 'Испытание scoped-отмены через отдельный broker и mock…\n'
CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" CAROSIGN_RELAY_CANCEL_TEST=1 \
    CAROSIGN_RELAY_TEST_RUNNER="$platform_dir/tools/run-test-relay.sh" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosign-broker --lib -- --exact \
        relay_pump::tests::separate_broker_initializes_real_mock \
        --ignored --nocapture

printf 'Положительная проверка с независимым испытательным проверяющим…\n'
CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::matched_mock_reaches_client_after_independent_verification_and_cleanup \
        --ignored --nocapture

printf 'Оценка утверждения без токена и доставка после уборки…\n'
CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::scoped_mock_verified_assertion_reaches_client_after_cleanup \
        --ignored --nocapture

printf 'Утверждение без токена через отдельные клиент, службу, broker и mock…\n'
CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" CAROSIGN_FLOW_TEST_TUNNEL=1 \
    CAROSIGN_FLOW_SEPARATE_CLIENT=1 \
    CAROSIGN_RELAY_TEST_RUNNER="$platform_dir/tools/run-test-relay.sh" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::tunnel_broker_scoped_assertion_reaches_client_after_cleanup \
        --ignored --nocapture

printf 'Положительная проверка с профилем испытательного broker…\n'
CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" CAROSIGN_FLOW_SEEDED_PROFILE=1 \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::matched_mock_reaches_client_after_independent_verification_and_cleanup \
        --ignored --nocapture

if [[ ${CAROSIGN_TEST_OWNED_HELPER:-0} == 1 ]]; then
printf 'Испытание owned-helper после cgroup и gate…\n'
if ! unshare --user --map-root-user --map-auto --mount --net \
    env CAROSIGN_TEST_OWNED_HELPER=1 CAROSIGN_SANDBOX_MOCK_BINARY="$mock_binary" \
    "$helper_binary" --exact process::root_switch::tests::confined_mock_performs_handshake \
    --nocapture | tee "$artifacts/owned-helper.log"; then
    printf 'Испытание owned-helper завершилось ошибкой.\n' >&2
    exit 1
fi
if ! grep -Fq 'Испытание owned-helper после cgroup и gate' "$artifacts/owned-helper.log" \
    || ! grep -Fq 'test result: ok. 1 passed' "$artifacts/owned-helper.log"; then
    printf 'Испытание owned-helper не было запущено.\n' >&2
    exit 1
fi
fi

printf 'Сквозной тест в отдельных user/mount/network namespaces…\n'
if ! unshare --user --map-root-user --map-auto --mount --net \
    env CAROSIGN_SANDBOX_MOCK_HELPER="$helper_binary" \
        CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib service_completion_tests -- --test-threads=1 | tee "$artifacts/test.log"; then
    printf 'Сквозной тест не выполнен или завершился ошибкой: проверьте user namespaces, subuid/subgid и результаты тестов.\n' >&2
    exit 1
fi
if ! grep -Fq 'test service_completion_tests::service_operation_reaches_client_after_mock_provider_and_confirmed_mock_cleanup ...' "$artifacts/test.log" \
    || ! grep -Fq 'running 1 test' "$artifacts/test.log"; then
    printf 'Сквозной тест или тестовый помощник не были запущены.\n' >&2
    exit 1
fi
printf 'Сквозной тест с отдельными процессами клиента и службы…\n'
if ! unshare --user --map-root-user --map-auto --mount --net \
    env CAROSIGN_SANDBOX_MOCK_HELPER="$helper_binary" \
        CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::public_flow_in_separate_service_and_client_processes \
        --ignored --nocapture --test-threads=1 | tee "$artifacts/process-test.log"; then
    printf 'Процессный сквозной тест не выполнен или завершился ошибкой.\n' >&2
    exit 1
fi
if ! grep -Fq 'test service_completion_tests::public_flow_in_separate_service_and_client_processes ...' "$artifacts/process-test.log" \
    || ! grep -Fq 'test service_completion_tests::public_client_process_fixture ... ok' "$artifacts/process-test.log"; then
    printf 'Отдельный клиент или процессный сценарий не были запущены.\n' >&2
    exit 1
fi
printf 'Сквозной тест с изолированным мок-процессом прошёл.\n'

printf 'Проверка PAM-клиента с отдельными процессами службы и мок-провайдера…\n'
if ! unshare --user --map-root-user --map-auto --mount --net \
    env CAROSIGN_TEST_PAM_OBSERVER=1 CAROSIGN_SANDBOX_MOCK_HELPER="$helper_binary" \
        CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::public_flow_in_separate_service_and_client_processes \
        --ignored --nocapture --test-threads=1 | tee "$artifacts/pam-process-test.log"; then
    printf 'Процессное испытание PAM-клиента не выполнено или завершилось ошибкой.\n' >&2
    exit 1
fi
if ! grep -Fq 'test service_completion_tests::public_client_process_fixture ... ok' "$artifacts/pam-process-test.log"; then
    printf 'Отдельный процесс PAM-клиента не был запущен.\n' >&2
    exit 1
fi
printf 'Процессное испытание PAM-клиента прошло.\n'

printf 'Проверка fprintd-фасада с отдельными процессами службы и мок-провайдера…\n'
for tool in dbus-run-session fprintd-verify; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Не найдено необходимое средство: %s\n' "$tool" >&2
        exit 1
    fi
done
printf 'Проверка fprintd через отдельные broker и mock…\n'
dbus-run-session -- env CAROSIGN_FLOW_TEST_TUNNEL=1 CAROSIGN_TEST_FPRINTD_ADAPTER=1 \
    CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    CAROSIGN_RELAY_TEST_RUNNER="$platform_dir/tools/run-test-relay.sh" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::tunnel_broker_mock_reaches_client_after_confirmed_cleanup \
        --ignored --nocapture
if ! unshare --user --map-root-user --map-auto --mount --net \
    dbus-run-session -- env CAROSIGN_TEST_FPRINTD_ADAPTER=1 CAROSIGN_SANDBOX_MOCK_HELPER="$helper_binary" \
        CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_binary" \
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
        -p carosignd --lib -- --exact \
        service_completion_tests::public_flow_in_separate_service_and_client_processes \
        --ignored --nocapture --test-threads=1 | tee "$artifacts/fprintd-process-test.log"; then
    printf 'Процессное испытание fprintd-фасада не выполнено или завершилось ошибкой.\n' >&2
    exit 1
fi
if ! grep -Fq 'test service_completion_tests::public_client_process_fixture ... ok' "$artifacts/fprintd-process-test.log"; then
    printf 'Отдельный процесс fprintd-фасада не был запущен.\n' >&2
    exit 1
fi
printf 'Процессное испытание fprintd-фасада прошло.\n'

printf 'Проверка VerifyStop через D-Bus и общий транспорт…\n'
dbus-run-session -- cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
    -p carosignd --lib -- --exact \
    service_completion_tests::fprintd_verify_stop_sends_cancel_to_socket_peer \
    --ignored --nocapture --test-threads=1
