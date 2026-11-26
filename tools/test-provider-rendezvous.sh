#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --locked -p carosign-broker
export CAROSIGN_TEST_BROKER="$PWD/target/debug/carosign-broker"
exec unshare --user --map-auto --map-root-user --mount --pid --fork sh -c '
    mount --make-rprivate /
    mount -t tmpfs -o mode=0755 tmpfs /run
    exec cargo test --offline --locked -p carosign-linux-ipc --test provider_rendezvous         open_session_preserves_worker_hello_and_rejects_bad_proof -- --ignored --exact
'
