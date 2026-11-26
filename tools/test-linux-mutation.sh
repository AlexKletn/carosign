#!/bin/sh
# Exercises Linux mutations with a separate mock provider that persists state between processes.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mock_dir=$(CDPATH= cd -- "$project_dir/crates/testing/carosign-mock-provider" && pwd)
cargo build --offline --manifest-path "$project_dir/Cargo.toml" -p carosign-broker -p carosign-fprintd-compat
cargo build --offline --manifest-path "$mock_dir/Cargo.toml"
cargo test --offline --manifest-path "$project_dir/Cargo.toml" -p carosignd --lib --no-run
exec unshare --user --map-root-user --mount sh -c '
    set -eu
    mount -t tmpfs -o mode=0755 tmpfs /run
    mkdir -p /run/carosignd
    printf "test resource" > /run/carosignd/test-resource
    cd "$1"
    unset XDG_RUNTIME_DIR
    exec dbus-run-session --config-file=crates/clients/carosign-fprintd-compat/tests/bus-test.conf -- \
        env CAROSIGN_TEST_PRIVATE_RUN=1 CAROSIGN_TEST_SCENARIO=linux-persistent \
        CAROSIGN_MOCK_STATE=/run/carosignd/mock-state \
        CAROSIGN_TEST_RESOURCE_PATH=/run/carosignd/test-resource \
        CAROSIGN_TEST_BROKER="$1/target/debug/carosign-broker" \
        CAROSIGN_TEST_FPRINTD="$1/target/debug/carosign-fprintd-compat" \
        CAROSIGN_TEST_PROVIDER="$2/target/debug/carosign-mock-provider" \
        cargo test --offline -p carosignd --lib -- \
            linux_mutation_e2e::enroll_verify_remove_with_separate_mock_processes --ignored --nocapture
' sh "$project_dir" "$mock_dir"
