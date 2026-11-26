#!/bin/sh
# Exercises the Linux stack and fprintd adapter with a separate mock provider.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mock_dir=$(CDPATH= cd -- "$project_dir/crates/testing/carosign-mock-provider" && pwd)
scenario=${1:-linux-matched-scope}
case "$scenario" in
    linux-matched-scope|linux-pending-cancel|linux-no-sample) ;;
    *) printf 'Неизвестный сценарий: %s\n' "$scenario" >&2; exit 2 ;;
esac
cargo build --offline --manifest-path "$project_dir/Cargo.toml" -p carosign-broker -p carosign-fprintd-compat
cargo build --offline --manifest-path "$mock_dir/Cargo.toml"
cargo test --offline --manifest-path "$project_dir/Cargo.toml" -p carosignd --lib --no-run
exec unshare --user --map-auto --map-root-user --mount sh -c '
    set -eu
    mount -t tmpfs tmpfs /run
    unset XDG_RUNTIME_DIR
    cd "$1"
    exec dbus-run-session --config-file=crates/clients/carosign-fprintd-compat/tests/bus-test.conf -- \
        env CAROSIGN_TEST_PRIVATE_RUN=1 CAROSIGN_TEST_SCENARIO="$3" \
        CAROSIGN_TEST_BROKER="$1/target/debug/carosign-broker" \
        CAROSIGN_TEST_FPRINTD="$1/target/debug/carosign-fprintd-compat" \
        CAROSIGN_TEST_PROVIDER="$2/target/debug/carosign-mock-provider" \
        cargo test --offline -p carosignd --lib -- \
            linux_adapter_e2e::real_fprintd_and_linux_service_with_mock --ignored --nocapture
' sh "$project_dir" "$mock_dir" "$scenario"
