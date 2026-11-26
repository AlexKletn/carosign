#!/bin/sh
set -eu
exec >/dev/console 2>&1

finish() {
    result=$?
    if [ "$result" -ne 0 ]; then
        systemctl status carosignd.service --no-pager || true
        printf 'CAROSIGN_SYSTEMD_FAILED\n'
    fi
    systemctl poweroff --force --force || true
}
trap finish EXIT

wait_for_socket() {
    for attempt in 1 2 3 4 5 6 7 8 9 10; do
        if /usr/bin/carosignctl health > /run/carosign-health 2>/dev/null; then
            grep -q '^служба: готова;' /run/carosign-health
            return
        fi
        sleep 1
    done
    return 1
}

systemctl is-enabled carosignd.service | grep -qx disabled
systemctl is-enabled carosign-fprintd.service | grep -qx disabled
test ! -e /var/lib/carosign/metadata.redb
test ! -e /etc/carosign/policy.conf
test ! -e /etc/carosign/providers.conf
test "$(stat -c '%u:%g:%a' /etc/carosign/carosignd.conf)" = 0:0:600
systemctl start carosignd.service
wait_for_socket
test -z "$(/usr/bin/carosignctl list)"
first_pid=$(systemctl show -p MainPID --value carosignd.service)
test "$first_pid" -gt 0
test "$(stat -c '%u:%g:%a' /run/carosignd)" = 0:0:755
test "$(stat -c '%u:%g:%a' /var/lib/carosign)" = 0:0:700
test "$(stat -c '%u:%g:%a' /var/lib/carosign/metadata.redb)" = 0:0:600
test "$(stat -c '%u:%g:%a' /etc/carosign/policy.conf)" = 0:0:600
test "$(stat -c '%u:%g:%a' /etc/carosign/providers.conf)" = 0:0:600
grep -qx 'uid=0 subject=0 actions=inventory' /etc/carosign/policy.conf
first_policy=$(stat -c '%d:%i' /etc/carosign/policy.conf)
first_providers=$(stat -c '%d:%i' /etc/carosign/providers.conf)
first_store=$(stat -c '%d:%i' /var/lib/carosign/metadata.redb)
systemctl restart carosignd.service
wait_for_socket
test "$(stat -c '%d:%i' /var/lib/carosign/metadata.redb)" = "$first_store"
test "$(stat -c '%d:%i' /etc/carosign/policy.conf)" = "$first_policy"
test "$(stat -c '%d:%i' /etc/carosign/providers.conf)" = "$first_providers"
second_pid=$(systemctl show -p MainPID --value carosignd.service)
test "$second_pid" -gt 0
test "$second_pid" != "$first_pid"
systemctl stop carosignd.service
test ! -e /run/carosignd/control.sock
printf 'corrupt fixture' > /var/lib/carosign/metadata.redb
cp /var/lib/carosign/metadata.redb /run/carosign-corrupt
systemctl start carosignd.service || true
sleep 1
test ! -S /run/carosignd/control.sock
cmp /run/carosign-corrupt /var/lib/carosign/metadata.redb
systemctl stop carosignd.service
printf 'CAROSIGN_SYSTEMD_PASSED\n'
