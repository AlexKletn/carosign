#!/bin/sh
# Builds the aarch64 review archive without installing it.
set -eu

if [ "$#" -ne 1 ]; then
    echo "Использование: sh $0 /абсолютный/путь/к/новому-архиву.tar" >&2
    exit 2
fi
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
sysroot=${CAROSIGN_SYSROOT:-/usr/aarch64-linux-gnu}
case "$sysroot" in
    /*) ;;
    *) echo 'CAROSIGN_SYSROOT должен быть абсолютным путём' >&2; exit 2 ;;
esac
if [ ! -f "$sysroot/usr/lib/libdbus-1.so" ]; then
    echo "Не найден AArch64 sysroot с libdbus: $sysroot; задайте CAROSIGN_SYSROOT" >&2
    exit 1
fi
test -f "$sysroot/usr/lib/libdbus-1.so"
test -f "$sysroot/usr/lib/libsystemd.so.0"
test -f "$sysroot/usr/lib/pkgconfig/dbus-1.pc"
test -f "$sysroot/usr/lib/pkgconfig/libsystemd.pc"

# The sysroot’s libm linker script contains absolute paths that can resolve to host libraries.
# Point pkg-config at a directory containing only the target libdbus.
# The installed aarch64 cross-toolchain supplies the system C libraries.
overlay=$(mktemp -d "${TMPDIR:-/tmp}/carosign-aarch64.XXXXXXXX")
trap 'rm -rf -- "$overlay"' EXIT HUP INT TERM
mkdir -p "$overlay/usr/lib/pkgconfig"
ln -s "$sysroot/usr/lib/libdbus-1.so" "$overlay/usr/lib/libdbus-1.so"
cp "$sysroot/usr/lib/pkgconfig/dbus-1.pc" "$sysroot/usr/lib/pkgconfig/libsystemd.pc" "$overlay/usr/lib/pkgconfig/"
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR="$overlay"
export PKG_CONFIG_LIBDIR="$overlay/usr/lib/pkgconfig"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-Wl,-rpath-link,$sysroot/usr/lib"
sh "$root/tools/build-review-bundle.sh" "$1" --target aarch64-unknown-linux-gnu
