#!/usr/bin/env bash
# Exercises the broker with a separate mock provider and tunnel client.
set -euo pipefail
platform_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
mock_dir=$(cd -- "$platform_dir/crates/testing/carosign-mock-provider" && pwd -P)
scratch=$(mktemp -d "${TMPDIR:-/tmp}/carosign-broker.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT

cargo build --offline --manifest-path "$platform_dir/Cargo.toml" -p carosign-broker
cargo build --offline --manifest-path "$mock_dir/Cargo.toml"
export CAROSIGN_REAL_BROKER="$platform_dir/target/debug/carosign-broker"
export CAROSIGN_MOCK_PROVIDER_TEST_BINARY="$mock_dir/target/debug/carosign-mock-provider"
export CAROSIGN_RELAY_TEST_RUNNER="$scratch/runner"
cat > "$CAROSIGN_RELAY_TEST_RUNNER" <<'RUNNER'
#!/bin/sh
# The fixture treats a closed client channel as a clean shutdown, which the broker reports
# with exit code 1. The client verifies whether the scenario succeeded.
"$CAROSIGN_REAL_BROKER" --tunnel-fd "$1" --provider-fd "$2" \
    --worker 01010101010101010101010101010101 --epoch 7 \
    --broker 03030303030303030303030303030303 --boot 04040404040404040404040404040404
code=$?
[ "$code" -eq 1 ]
RUNNER
chmod 700 "$CAROSIGN_RELAY_TEST_RUNNER"
for scenario in normal matched cancel mismatch; do
    printf 'Проверка отдельного посредника: %s\n' "$scenario"
    unset CAROSIGN_RELAY_MATCHED_TEST CAROSIGN_RELAY_CANCEL_TEST CAROSIGN_RELAY_MISMATCH_TEST
    case "$scenario" in
        matched) export CAROSIGN_RELAY_MATCHED_TEST=1 ;;
        cancel) export CAROSIGN_RELAY_CANCEL_TEST=1 ;;
        mismatch) export CAROSIGN_RELAY_MISMATCH_TEST=1 ;;
    esac
    cargo test --offline --manifest-path "$platform_dir/Cargo.toml" -p carosign-broker --lib \
        relay_pump::tests::separate_broker_initializes_real_mock -- --ignored --nocapture
done

printf 'Проверка регистрации и удаления через настоящий broker и mock\n'
cargo test --offline --manifest-path "$platform_dir/Cargo.toml" -p carosign-broker \
    --test process_mutation process_real_mock_enrollment_and_removal -- --ignored --nocapture

# The supervisor clears the worker environment, so the temporary launcher selects
# the test scenario through a command-line argument.
printf '#!/usr/bin/env bash\nexec %q "$1" seeded-no-sample\n' \
    "$CAROSIGN_MOCK_PROVIDER_TEST_BINARY" > "$scratch/provider"
chmod 755 "$scratch/provider"
export CAROSIGN_TEST_BROKER="$CAROSIGN_REAL_BROKER"
export CAROSIGN_TEST_PROVIDER="$scratch/provider"
printf 'Проверка supervisor, выдачи и освобождения ресурсов\n'
cargo test --offline --manifest-path "$platform_dir/Cargo.toml" -p carosignd --lib \
    supervised_verification::tests::real_supervisor_lease_broker_and_mock -- --ignored --nocapture
printf 'Проверка подготовки канала для VerificationFlow\n'
CAROSIGN_TEST_PREPARE_SCOPE=1 cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
    -p carosignd --lib supervised_verification::tests::real_supervisor_lease_broker_and_mock -- --ignored --nocapture
