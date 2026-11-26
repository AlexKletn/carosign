#!/bin/sh
set -eu

# Uses an isolated PAM configuration and a fixture password module.
# The system PAM configuration and accounts remain untouched.
cd "$(dirname "$0")/.."
task_pam_target=${CARGO_TARGET_DIR:-$(pwd)/target}
export CARGO_TARGET_DIR="$task_pam_target"
cargo build -p pam-carosign --lib --offline
task_pam_tmp=$(mktemp -d)
trap 'rm -rf "$task_pam_tmp"' EXIT HUP INT TERM
task_pam_module=$task_pam_target/debug/libpam_carosign.so
task_pam_fixture=$task_pam_tmp/pam_fixture.so
cc -std=c11 -D_GNU_SOURCE -Wall -Wextra -Werror -fPIC -shared \
    crates/clients/pam-carosign/tests/fixture_password.c -o "$task_pam_fixture" -lpam
cc -std=c11 -D_GNU_SOURCE -Wall -Wextra -Werror \
    crates/clients/pam-carosign/tests/fixture_pam_stack.c -o "$task_pam_tmp/check" -lpam
printf 'auth required %s\n' "$task_pam_module" > "$task_pam_tmp/carosign-only"
printf 'auth [authinfo_unavail=ignore default=die] %s\nauth required %s password\n' \
    "$task_pam_module" "$task_pam_fixture" > "$task_pam_tmp/password"
printf 'auth optional %s seed\nauth [authinfo_unavail=ignore default=die] %s\nauth required %s check\n' \
    "$task_pam_fixture" "$task_pam_module" "$task_pam_fixture" > "$task_pam_tmp/token"
"$task_pam_tmp/check" "$task_pam_tmp"
