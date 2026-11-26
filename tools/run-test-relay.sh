#!/usr/bin/env bash
# Runs only the test harness.
set -euo pipefail

platform_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
if [[ $# != 2 || ! $1 =~ ^[0-9]+$ || ! $2 =~ ^[0-9]+$ || $1 == "$2" ]]; then
    printf 'Использование: %s <tunnel-fd> <provider-fd>\n' "$0" >&2
    exit 2
fi
for relay_fd in "$1" "$2"; do
    if [[ ! -e /proc/$$/fd/$relay_fd ]]; then
        printf 'Нет переданного дескриптора: %s\n' "$relay_fd" >&2
        exit 2
    fi
done

artifacts=$(mktemp -d "${TMPDIR:-/tmp}/carosign-test-relay.XXXXXXXX")
trap 'rm -rf -- "$artifacts"' EXIT
cargo test --offline --manifest-path "$platform_dir/Cargo.toml" \
    -p carosign-broker --lib --no-run --message-format=json > "$artifacts/build.json"
relay_binary=$(python3 - "$artifacts/build.json" <<'PY'
import json
import os
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    records = [json.loads(line) for line in stream]
paths = {r["executable"] for r in records
         if r.get("reason") == "compiler-artifact"
         and r.get("target", {}).get("name") == "carosign_broker"
         and "lib" in r.get("target", {}).get("kind", [])
         and r.get("profile", {}).get("test")
         and r.get("executable")}
if len(paths) != 1 or not os.path.isfile(next(iter(paths), "")):
    raise SystemExit("Не найден единственный испытательный бинарник broker")
print(os.path.realpath(paths.pop()))
PY
)
unset CAROSIGN_INIT_RELAY_CLIENT
CAROSIGN_INIT_RELAY_TUNNEL=$1 CAROSIGN_INIT_RELAY_PROVIDER=$2 \
    "$relay_binary" --exact relay_pump::tests::separate_broker_initializes_real_mock \
    --ignored --nocapture
