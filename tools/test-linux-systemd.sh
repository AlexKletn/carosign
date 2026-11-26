#!/bin/sh
set -eu
project=$(CDPATH='' cd "$(dirname "$0")/.." && pwd)
kernel=${1:-/boot/vmlinuz-linux}
test "$(uname -m)" = x86_64
test -r "$kernel"
mkdir -p "$project/target"
work=$(mktemp -d "$project/target/systemd-acceptance.XXXXXXXX")
trap 'rm -rf "$work"' EXIT
trap 'exit 130' HUP INT TERM
export PKGDEST="$work"

cd "$project"
cargo build --locked --offline --release -p carosignd -p carosign-broker -p carosignctl \
    -p carosign-fprintd-compat -p pam-carosign
sed "s/^arch=.*/arch=('$(uname -m)')/" packaging/arch/PKGBUILD > "$work/PKGBUILD"
(
    cd "$work"
    CAROSIGN_BUILD_DIR="$project/target/release" CAROSIGN_PROJECT_DIR="$project" \
        makepkg --nodeps --nosign --noconfirm
)
package=$(find "$work" -maxdepth 1 -name '*.pkg.tar.zst' -print)
test -f "$package"
unshare --user --map-root-user --mount sh -s -- "$project" "$work" "$package" <<'GUEST'
set -eu
project=$1
work=$2
package=$3
root=$work/root
mkdir -p "$root/usr/bin" "$root/usr/lib/systemd/system" "$root/usr/lib64" \
    "$root/etc/carosign" "$root/var/lib/pacman" "$root/proc" \
    "$root/sys" "$root/dev" "$root/run" "$root/tmp" "$work/cache" "$work/hooks"
ln -s usr/bin "$root/bin"
ln -s usr/lib "$root/lib"
ln -s usr/lib64 "$root/lib64"
printf '[options]\nArchitecture = auto\nSigLevel = Never\n' > "$work/pacman.conf"
pacman --config "$work/pacman.conf" --root "$root" --dbpath "$root/var/lib/pacman" \
    --cachedir "$work/cache" --hookdir "$work/hooks" --noconfirm \
    --assume-installed glibc --assume-installed libgcc --assume-installed dbus \
    --assume-installed pam --assume-installed systemd -U "$package"

copy_binary() {
    path=$1
    install -D -m 0755 "$path" "$root$path"
    ldd "$path" | awk '$2 == "=>" && $3 ~ /^\// { print $3 }' |
        while IFS= read -r library; do
            install -D -m 0755 "$library" "$root$library"
        done
}
for binary in systemctl sh grep sleep install stat cp cmp mount; do
    copy_binary "/usr/bin/$binary"
done
copy_binary /usr/lib/systemd/systemd
copy_binary /usr/lib/systemd/systemd-executor
copy_binary /usr/lib/systemd/systemd-shutdown
copy_binary /usr/lib/libkmod.so.2
copy_binary /usr/lib/libbpf.so.1
install -D -m 0755 /usr/lib/libseccomp.so.2 "$root/usr/lib/libseccomp.so.2"
install -D -m 0755 /usr/lib64/ld-linux-x86-64.so.2 "$root/usr/lib64/ld-linux-x86-64.so.2"
for unit in sysinit basic local-fs local-fs-pre shutdown; do
    cp "/usr/lib/systemd/system/$unit.target" "$root/usr/lib/systemd/system/"
done
printf 'NAME="Biometric acceptance"\nID=carosign-test\n' > "$root/etc/os-release"
printf 'root:x:0:0:Root:/root:/bin/sh\n' > "$root/etc/passwd"
printf 'root:x:0:\n' > "$root/etc/group"
install -m 0755 "$project/crates/linux/carosignd/tests/support/systemd-acceptance.sh" \
    "$root/usr/bin/carosign-acceptance"
mkdir -p "$root/etc/systemd/system/carosignd.service.d"
cat > "$root/etc/systemd/system/carosignd.service.d/console.conf" <<'CONSOLE'
[Service]
StandardOutput=tty
StandardError=tty
TTYPath=/dev/console
CONSOLE
cat > "$root/usr/lib/systemd/system/carosign-acceptance.service" <<'UNIT'
[Unit]
DefaultDependencies=no
[Service]
Type=oneshot
ExecStart=/usr/bin/carosign-acceptance
UNIT
(
    cd "$root"
    find . -print0 | cpio --null -o --format=newc --quiet | gzip -1 > "$work/initramfs"
)
GUEST

if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
    acceleration=kvm
    cpu=host
else
    acceleration=tcg
    cpu=max
fi
timeout 90 qemu-system-x86_64 -accel "$acceleration" -cpu "$cpu" -m 1024 -nodefaults -nic none \
    -no-reboot -display none -serial stdio -monitor none \
    -kernel "$kernel" -initrd "$work/initramfs" \
    -append 'console=ttyS0 random.trust_cpu=on rdinit=/usr/lib/systemd/systemd systemd.unit=carosign-acceptance.service systemd.log_target=console systemd.log_level=info systemd.show_status=no' \
    > "$work/console" 2>&1 || { cat "$work/console"; exit 1; }
cat "$work/console"
grep -q '^CAROSIGN_SYSTEMD_PASSED' "$work/console"
