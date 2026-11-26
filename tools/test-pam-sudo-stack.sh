#!/bin/sh
# Checks the new stack order with an isolated PAM configuration directory.
set -eu
cd "$(dirname "$0")/.."
task_sudo_tmp=$(mktemp -d)
trap 'rm -rf "$task_sudo_tmp"' EXIT HUP INT TERM
cc -std=c11 -D_GNU_SOURCE -Wall -Wextra -Werror -fPIC -shared \
    crates/clients/pam-carosign/tests/fixture_sudo_module.c -o "$task_sudo_tmp/mock.so" -lpam
cc -std=c11 -D_GNU_SOURCE -Wall -Wextra -Werror -fPIC -shared \
    crates/clients/pam-carosign/tests/fixture_password.c -o "$task_sudo_tmp/password.so" -lpam
cc -std=c11 -D_GNU_SOURCE -Wall -Wextra -Werror \
    crates/clients/pam-carosign/tests/fixture_sudo_stack.c -o "$task_sudo_tmp/check" -lpam
cc -std=c11 -D_GNU_SOURCE -Wall -Wextra -Werror \
    crates/clients/pam-carosign/tests/fixture_faillock_biometrics.c -o "$task_sudo_tmp/check-faillock" -lpam
printf 'auth required %s password\n' "$task_sudo_tmp/password.so" > "$task_sudo_tmp/fixture-password"
for scenario in matched unavailable no-match locked; do
    case "$scenario" in
        matched) preauth=preauth; bio=bio-match ;;
        unavailable) preauth=preauth; bio=bio-unavailable ;;
        no-match) preauth=preauth; bio=bio-no-match ;;
        locked) preauth=locked; bio=bio-match ;;
    esac
    printf 'auth requisite %s %s\nauth [success=2 auth_err=ignore default=1] %s %s\nauth [default=ignore] %s authfail\nauth [default=3] pam_permit.so\nauth requisite %s authsucc\nauth required %s env\nauth sufficient pam_permit.so\nauth include fixture-password\n' \
        "$task_sudo_tmp/mock.so" "$preauth" "$task_sudo_tmp/mock.so" "$bio" \
        "$task_sudo_tmp/mock.so" \
        "$task_sudo_tmp/mock.so" "$task_sudo_tmp/mock.so" > "$task_sudo_tmp/$scenario"
done
export CAROSIGN_TEST_PAM_TRACE="$task_sudo_tmp/trace"
"$task_sudo_tmp/check" "$task_sudo_tmp"
mkdir "$task_sudo_tmp/tally"
printf 'auth requisite pam_faillock.so preauth deny=2 dir=%s/tally\nauth [success=2 auth_err=ignore default=1] %s bio-no-match\nauth [default=ignore] pam_faillock.so authfail deny=2 dir=%s/tally\nauth [default=3] pam_permit.so\nauth requisite pam_faillock.so authsucc deny=2 dir=%s/tally\nauth required %s env\nauth sufficient pam_permit.so\nauth include fixture-password\n' \
    "$task_sudo_tmp" "$task_sudo_tmp/mock.so" "$task_sudo_tmp" "$task_sudo_tmp" "$task_sudo_tmp/mock.so" > "$task_sudo_tmp/real-faillock"
"$task_sudo_tmp/check-faillock" "$task_sudo_tmp"
