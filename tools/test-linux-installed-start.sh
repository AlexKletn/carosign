#!/bin/sh
# First launch of the production entry point in a disposable native root.
# This test does not start systemd, install the package, or exercise aarch64.
set -eu

project_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
mock_dir=$(CDPATH='' cd -- "$project_dir/crates/testing/carosign-mock-provider" && pwd)
cargo build --offline --manifest-path "$project_dir/Cargo.toml" \
    -p carosignd -p carosignctl -p carosign-broker -p carosign-fprintd-compat -p pam-carosign
cargo build --offline --manifest-path "$mock_dir/Cargo.toml"

exec unshare --user --map-auto --map-root-user --mount --pid --fork sh -s -- \
    "$project_dir" "$mock_dir" "${1:-}" <<'SCRIPT'
set -eu
project_dir=$1
mock_dir=$2
mode=$3
work=$(mktemp -d /tmp/carosign-start-test.XXXXXXXX)
root=$work/root
daemon_pid=
provider_pid=
second_pid=
mkdir -p "$root"
mount -t tmpfs -o mode=0755 tmpfs "$root"
stop_provider() {
    if [ -n "$provider_pid" ]; then
        kill "$provider_pid" 2>/dev/null || true
        wait "$provider_pid" 2>/dev/null || true
        provider_pid=
        rm -f "$publication/control.sock"
    fi
}
cleanup() {
    stop_provider
    if [ -n "$second_pid" ]; then
        kill "$second_pid" 2>/dev/null || true
        wait "$second_pid" 2>/dev/null || true
    fi
    if [ -n "$daemon_pid" ]; then
        kill "$daemon_pid" 2>/dev/null || true
        wait "$daemon_pid" 2>/dev/null || true
    fi
    umount /run 2>/dev/null || true
    umount -R "$root" 2>/dev/null || umount -l "$root"
    rmdir "$root"
    rm -rf "$work"
}
trap cleanup EXIT HUP INT TERM
mkdir -p "$root/usr/bin" "$root/usr/lib" "$root/usr/lib64" \
    "$root/etc/carosign" "$root/var/lib/carosign" \
    "$root/var/lib/carosign-mock" "$root/run/carosignd" "$root/proc" "$root/dev" "$root/bin"
mount -t proc proc "$root/proc"
mount --bind "$root/run" /run
touch "$root/dev/null"
mount --bind /dev/null "$root/dev/null"
for name in carosignd carosignctl carosign-broker carosign-config; do
    cp "$project_dir/target/debug/$name" "$root/usr/bin/$name"
done
cp "$mock_dir/target/debug/carosign-mock-provider" "$root/usr/bin/carosign-mock-provider"
cp -L /usr/bin/sh "$root/usr/bin/sh"
ln -s ../usr/bin/sh "$root/bin/sh"
# Requires an x86_64 Arch host. Library copies remain in tmpfs.
cp -L /usr/lib/libgcc_s.so.1 /usr/lib/libc.so.6 /usr/lib/libreadline.so.8 \
    /usr/lib/libncursesw.so.6 "$root/usr/lib/"
cp -L /usr/lib64/ld-linux-x86-64.so.2 "$root/usr/lib64/"
ln -s usr/lib "$root/lib"
ln -s usr/lib64 "$root/lib64"
printf 'root:x:0:0:root:/root:/bin/sh\nnobody:x:65534:65534:nobody:/:/bin/false\n' > "$root/etc/passwd"
printf 'root:x:0:\nnobody:x:65534:\n' > "$root/etc/group"
printf 'uid=0 subject=0 actions=verify,enroll,remove,inventory purpose=session-login\nuid=65533 subject=65533 actions=inventory\n' > "$root/etc/carosign/policy.conf"
cat > "$root/etc/carosign/provider.conf" <<'PROFILE'
broker=/usr/bin/carosign-broker
provider=/usr/bin/carosign-mock-wrapper
provider_uid=65534
provider_gid=65534
package=01010101010101010101010101010101
profile=02020202020202020202020202020202
authenticator=03030303030303030303030303030303
resource=04040404040404040404040404040404
modality=fingerprint
PROFILE
cat > "$root/usr/bin/carosign-mock-wrapper" <<'WRAPPER'
#!/bin/sh
export CAROSIGN_MOCK_STATE=/var/lib/carosign-mock/state
exec /usr/bin/carosign-mock-provider "$1" linux-persistent
WRAPPER
chmod 755 "$root/usr/bin/carosign-mock-wrapper"
chown 65534:65534 "$root/var/lib/carosign-mock"
chmod 700 "$root/var/lib/carosign-mock"
cat > "$root/etc/carosign/carosignd.conf" <<'CONFIG'
socket=/run/carosignd/control.sock
store=/var/lib/carosign/metadata.redb
policy=/etc/carosign/policy.conf
provider_profile=/etc/carosign/provider.conf
CONFIG
if [ "$mode" = "--discovered" ] || [ "$mode" = "--two-providers" ] || [ "$mode" = "--sudo" ]; then
    sed -i '/^provider_profile=/d' "$root/etc/carosign/carosignd.conf"
    printf 'demo 65534 65534 03030303030303030303030303030303 /etc/carosign/provider.conf\n' \
        > "$root/etc/carosign/providers.conf"
    publication=/run/carosign/providers/demo
    mkdir -p "$publication"
    chown 65534:65534 "$publication"
    chmod 755 "$work"
    printf '#!/bin/sh\nexit 123\n' > "$root/usr/bin/carosign-mock-wrapper"
fi
launch_provider() {
    provider_socket=$1
    python3 "$project_dir/crates/linux/carosignd/tests/support/discovered_provider.py" \
        "$provider_socket" "$mock_dir/target/debug/carosign-mock-provider" "$2" "$3" "$4" "$5" &
    launched_pid=$!
    for attempt in $(seq 50); do
        test ! -S "$provider_socket" || return 0
        kill -0 "$launched_pid"
        sleep 0.02
    done
    return 1
}
start_provider() {
    launch_provider "$publication/control.sock" "$root/var/lib/carosign-mock/state" \
        "${1:-65534}" 03030303030303030303030303030303 2
    provider_pid=$launched_pid
}
if [ "$mode" = "--two-providers" ]; then
    second=/run/carosign/providers/second
    mkdir -p "$second" "$root/var/lib/carosign-mock-second"
    chown 65533:65533 "$second" "$root/var/lib/carosign-mock-second"
    chmod 700 "$root/var/lib/carosign-mock-second"
    sed -e 's/65534/65533/g' -e 's/03030303030303030303030303030303/05050505050505050505050505050505/' \
        -e 's/04040404040404040404040404040404/06060606060606060606060606060606/' \
        "$root/etc/carosign/provider.conf" > "$root/etc/carosign/second.conf"
    printf 'second 65533 65533 05050505050505050505050505050505 /etc/carosign/second.conf\n' \
        >> "$root/etc/carosign/providers.conf"
    launch_provider "$second/control.sock" "$root/var/lib/carosign-mock-second/state" \
        65533 05050505050505050505050505050505 1
    second_pid=$launched_pid
fi
if [ "$mode" = "--sudo" ]; then
    . "$project_dir/crates/linux/carosignd/tests/support/installed-sudo.sh"
    install_sudo_fixture
fi
chmod 600 "$root/etc/carosign/"*.conf
mkdir "$work/pam"
printf 'auth required %s\n' "$project_dir/target/debug/libpam_carosign.so" > "$work/pam/carosign-test"
cat > "$work/pam-test.c" <<'PAM_TEST'
#include <security/pam_appl.h>
#include <stdio.h>
static int conversation(int n, const struct pam_message **messages,
                        struct pam_response **responses, void *context) {
    (void)n; (void)messages; (void)responses; (void)context;
    return PAM_CONV_ERR;
}
int main(int argc, char **argv) {
    if (argc != 2) return 2;
    struct pam_conv conv = { conversation, NULL };
    pam_handle_t *handle = NULL;
    int result = pam_start_confdir("carosign-test", "root", &conv, argv[1], &handle);
    if (result == PAM_SUCCESS) result = pam_authenticate(handle, 0);
    if (result == PAM_SUCCESS) result = pam_setcred(handle, PAM_ESTABLISH_CRED);
    if (handle) pam_end(handle, result);
    if (result != PAM_SUCCESS) fprintf(stderr, "PAM: %d\n", result);
    return result == PAM_SUCCESS ? 0 : 1;
}
PAM_TEST
cc -D_GNU_SOURCE -Wall -Wextra -Werror "$work/pam-test.c" -o "$work/pam-test" -lpam
chroot "$root" /usr/bin/carosignd init-store /var/lib/carosign/manual.redb
cp "$root/var/lib/carosign/manual.redb" "$work/original-store"
if chroot "$root" /usr/bin/carosignd init-store /var/lib/carosign/manual.redb 2>/dev/null; then
    printf 'повторная инициализация неожиданно прошла\n' >&2
    exit 1
fi
cmp "$work/original-store" "$root/var/lib/carosign/manual.redb"
test ! -e "$root/var/lib/carosign/metadata.redb"
chroot "$root" /usr/bin/carosignd --config /etc/carosign/carosignd.conf \
    >"$work/stdout" 2>"$work/stderr" &
daemon_pid=$!
attempt=0
while [ ! -S "$root/run/carosignd/control.sock" ]; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 50 ] || ! kill -0 "$daemon_pid" 2>/dev/null; then
        cat "$work/stderr" >&2
        printf 'служба не опубликовала сокет\n' >&2
        exit 1
    fi
    sleep 0.1
done
test "$(stat -c '%u:%g:%a' "$root/var/lib/carosign/metadata.redb")" = 0:0:600
if [ "$mode" = "--discovered" ] || [ "$mode" = "--two-providers" ] || [ "$mode" = "--sudo" ]; then
    if [ "$mode" = "--two-providers" ]; then
        test "$(chroot "$root" /usr/bin/carosignctl list)" = "05050505  доступен"
    else
        test -z "$(chroot "$root" /usr/bin/carosignctl list)"
    fi
    start_provider
    for attempt in $(seq 100); do
        test ! -f "$root/var/lib/carosign-mock/state.described" || break
        sleep 0.02
    done
    test -f "$root/var/lib/carosign-mock/state.described"
    chroot "$root" /usr/bin/carosignctl list | grep -q '03030303.* доступен'
    stop_provider
    chroot "$root" /usr/bin/carosignctl list | grep -q '03030303.*недоступен'
    if [ "$mode" = "--two-providers" ]; then
        start_provider 0
        chown 65534:65534 "$publication/control.sock"
        chroot "$root" /usr/bin/carosignctl list > "$work/foreign-inventory"
        grep -q '03030303.*недоступен' "$work/foreign-inventory"
        grep -q '05050505.* доступен' "$work/foreign-inventory"
        stop_provider
    fi
    start_provider
fi
chroot "$root" /usr/bin/carosignctl list | grep -q '03030303'
if [ "$mode" = "--headless" ]; then
    chroot "$root" /usr/bin/carosign-config check service
    chroot "$root" /usr/bin/carosign-config check policy
    chroot "$root" /usr/bin/carosign-config check profile
    chroot "$root" /usr/bin/carosign-config show service | grep -q 'provider_profile='
    chroot "$root" /usr/bin/carosign-config diagnose > "$work/headless-diagnostics"
    grep -q 'service_ready=true' "$work/headless-diagnostics"
    sample=$(chroot "$root" /usr/bin/carosignctl enroll - left-index-finger 03030303)
    test "${#sample}" = 32
    chroot "$root" /usr/bin/carosignctl list enrollments | grep -q "$sample"
    test "$(chroot "$root" /usr/bin/carosignctl remove "$sample")" = "$sample"
    chroot "$root" /usr/bin/carosignctl list enrollments > "$work/headless-enrollments"
    if grep -q "$sample" "$work/headless-enrollments"; then
        exit 1
    fi
    printf 'Headless enrollment, removal and configuration diagnostics passed.\n'
    
fi
chroot --userspec=65533:65533 "$root" /usr/bin/carosignctl list | grep -q '03030303'
chroot --userspec=65534:65534 "$root" /usr/bin/carosignctl health > "$work/unprivileged-health"
if chroot --userspec=65534:65534 "$root" /usr/bin/carosignctl list > "$work/denied-inventory" 2>&1; then
    printf 'инвентарь выдан без разрешения политики\n' >&2
    exit 1
fi
grep -q 'служба отказала' "$work/denied-inventory"
run_client_cycle() {
    bus_config="$project_dir/crates/clients/carosign-fprintd-compat/tests/bus-test.conf"
    if [ "$mode" = "--sudo" ]; then
        sed 's|unix:tmpdir=/tmp|unix:path=/run/carosignd/sudo-test-bus|' "$bus_config" > "$work/sudo-bus.conf"
        bus_config="$work/sudo-bus.conf"
    fi
    dbus-run-session --config-file="$bus_config" -- \
    sh -s -- "$project_dir" "$work" "$root" "$mode" <<'DBUS_TEST'
set -eu
export DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS
if [ "$4" = "--sudo" ]; then
    chroot "$3" /usr/bin/carosign-fprintd-compat &
else
    "$1/target/debug/carosign-fprintd-compat" &
fi
adapter_pid=$!
trap 'kill "$adapter_pid" 2>/dev/null || true; wait "$adapter_pid" 2>/dev/null || true' EXIT
attempt=0
while ! fprintd-enroll -f left-thumb root > "$2/enroll-output" 2>&1; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 30 ] || ! kill -0 "$adapter_pid" 2>/dev/null; then
        cat "$2/enroll-output" >&2
        exit 1
    fi
    sleep 0.1
done
grep -q 'enroll-completed' "$2/enroll-output"
"$2/pam-test" "$2/pam"
fprintd-verify -f any root | grep -q 'verify-match'
if [ "$4" = "--fprintd-contract" ]; then
    fprintd-enroll -f right-index-finger root > "$2/enroll-second"
    grep -q 'enroll-completed' "$2/enroll-second"
    fprintd-verify -f right-index-finger root > "$2/verify-selected" 2>&1 || { cat "$2/verify-selected" "$2/stderr" >&2; exit 1; }
    grep -q 'verify-match' "$2/verify-selected" || { cat "$2/verify-selected" "$2/stderr" >&2; exit 1; }
    gdbus call --system --dest net.reactivated.Fprint \
        --object-path /net/reactivated/Fprint/Device/0 \
        --method net.reactivated.Fprint.Device.DeleteEnrolledFingers root
    if fprintd-list root > "$2/after-legacy-delete" 2>&1; then
        if grep -q 'right-index-finger' "$2/after-legacy-delete"; then
            exit 1
        fi
    fi
    fprintd-enroll -f left-thumb root > "$2/enroll-after-legacy-delete"
    grep -q 'enroll-completed' "$2/enroll-after-legacy-delete"
fi
if [ "$4" = "--long-running" ]; then
    CAROSIGN_LONG_RUN_SOCKET=/run/carosignd/control.sock \
        cargo test --locked --offline --manifest-path "$1/Cargo.toml" -p carosignd \
        --test long_running_verification -- --ignored || {
        tail -80 "$2/stderr" >&2
        exit 1
    }
fi
fprintd-delete root
if [ "$4" = "--sudo" ]; then
    project_dir=$1
    root=$3
    . "$project_dir/crates/linux/carosignd/tests/support/installed-sudo.sh"
    fprintd-enroll -f left-thumb biotest >"$2/sudo-enrollment" 2>&1 || {
        cat "$2/sudo-enrollment" "$2/stderr" >&2
        exit 1
    }
    grep -q enroll-completed "$2/sudo-enrollment" || { cat "$2/sudo-enrollment" >&2; exit 1; }
    test "$(run_sudo </dev/null)" = 0
fi
DBUS_TEST
}
run_client_cycle
if [ "$mode" = "--sudo" ]; then
    stop_provider
    if run_sudo </dev/null >"$work/sudo-denied" 2>&1; then
        printf 'sudo unexpectedly succeeded without either factor\n' >&2
        exit 1
    fi
    test "$(printf 'fixture-password\n' | run_sudo)" = 0
    start_provider
    printf 'sudo: biometric factor and fallback verified.\n'
fi

if [ "$mode" = "--two-providers" ]; then
    test -f "$root/var/lib/carosign-mock/state"
    stop_provider
    chroot "$root" /usr/bin/carosignctl list > "$work/failover-inventory"
    grep -q '03030303.*недоступен' "$work/failover-inventory"
    grep -q '05050505.* доступен' "$work/failover-inventory"
    run_client_cycle
    test -f "$root/var/lib/carosign-mock-second/state"
    start_provider
fi
kill "$daemon_pid"
wait "$daemon_pid" 2>/dev/null || true
daemon_pid=
# systemd removes RuntimeDirectory on shutdown. This test runs without systemd,
# so it removes the remaining socket explicitly.
rm "$root/run/carosignd/control.sock"
chroot "$root" /usr/bin/carosignd --config /etc/carosign/carosignd.conf \
    >"$work/stdout-restart" 2>"$work/stderr-restart" &
daemon_pid=$!
attempt=0
while [ ! -S "$root/run/carosignd/control.sock" ]; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 50 ] || ! kill -0 "$daemon_pid" 2>/dev/null; then
        cat "$work/stderr-restart" >&2
        printf 'служба не перезапустилась после мутаций\n' >&2
        exit 1
    fi
    sleep 0.1
done
chroot "$root" /usr/bin/carosignctl list | grep -q '03030303'
kill "$daemon_pid"
wait "$daemon_pid" 2>/dev/null || true
daemon_pid=
rm "$root/run/carosignd/control.sock"
# The disposable root lets us exercise corrupt storage without touching live data.
# The service must reject the store and preserve its contents.
printf 'corrupted-store' > "$root/var/lib/carosign/metadata.redb"
cp "$root/var/lib/carosign/metadata.redb" "$work/corrupt-store"
if chroot "$root" /usr/bin/carosignd --config /etc/carosign/carosignd.conf \
    >"$work/stdout-corrupt" 2>"$work/stderr-corrupt"; then
    printf 'служба приняла повреждённое хранилище\n' >&2
    exit 1
fi
test ! -e "$root/run/carosignd/control.sock"
cmp "$work/corrupt-store" "$root/var/lib/carosign/metadata.redb"
printf 'Production-точка входа: fprintd регистрация, PAM/fprintd проверка, удаление и перезапуск прошли; данные при ошибке сохранены.\n'
SCRIPT
