#!/bin/sh
# Exercises interruptions after arming and checks that uncertain scopes remain fenced.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mock_dir=$(CDPATH= cd -- "$project_dir/crates/testing/carosign-mock-provider" && pwd)
cargo build --offline --manifest-path "$project_dir/Cargo.toml" -p carosign-broker
cargo build --offline --manifest-path "$mock_dir/Cargo.toml"
for test_name in mutation_cancel_keeps_scope_for_reconciliation mutation_disconnect_keeps_scope_for_reconciliation mutation_deadline_keeps_scope_for_reconciliation; do
    CAROSIGN_TEST_BROKER="$project_dir/target/debug/carosign-broker" \
    CAROSIGN_TEST_PROVIDER="$mock_dir/target/debug/carosign-mock-provider" \
    CAROSIGN_TEST_SCENARIO=linux-pending-mutation \
        cargo test --offline --manifest-path "$project_dir/Cargo.toml" -p carosignd \
        "$test_name" -- --ignored --nocapture
done

for scenario in linux-pending-rollback linux-lost-rollback linux-committed-rollback linux-error-rollback; do
    case "$scenario" in
        linux-pending-rollback) test_name=mutation_confirmed_rollback_releases_scope ;;
        linux-lost-rollback) test_name=mutation_lost_rollback_keeps_scope_for_reconciliation ;;
        linux-committed-rollback) test_name=mutation_committed_before_rollback_keeps_scope_for_reconciliation ;;
        linux-error-rollback) test_name=mutation_provider_error_rolls_back_and_allows_retry ;;
    esac
    CAROSIGN_TEST_BROKER="$project_dir/target/debug/carosign-broker" \
    CAROSIGN_TEST_PROVIDER="$mock_dir/target/debug/carosign-mock-provider" \
    CAROSIGN_TEST_SCENARIO="$scenario" \
        cargo test --offline --manifest-path "$project_dir/Cargo.toml" -p carosignd \
        "$test_name" -- --ignored --nocapture
done

CAROSIGN_TEST_BROKER="$project_dir/target/debug/carosign-broker" \
CAROSIGN_TEST_PROVIDER="$mock_dir/target/debug/carosign-mock-provider" \
CAROSIGN_TEST_SCENARIO=linux-pending-rollback \
    cargo test --offline --manifest-path "$project_dir/Cargo.toml" -p carosignd \
    system_client_cancel_uses_live_rollback_and_releases_scope -- --ignored --nocapture
