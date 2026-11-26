#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
    echo "Usage: CAROSIGN_SYSROOT=/path/to/aarch64/sysroot sh $0 /path/to/new-package.pkg.tar.zst" >&2
    exit 2
fi
case "$1" in /*) output=$1 ;; *) echo 'Use an absolute output path' >&2; exit 2 ;; esac
project=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
sysroot=$(realpath "${CAROSIGN_SYSROOT:?Set CAROSIGN_SYSROOT}")
client="$project/crates/clients/carosign-ui"
target=aarch64-unknown-linux-gnu
build="$client/target/$target/release"
parent=$(dirname -- "$output")
test -d "$parent"
test ! -e "$output"
test ! -L "$output"

work=$(mktemp -d "$parent/.carosign-ui-arch.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT HUP INT TERM
(
    cd "$project"
    export PKG_CONFIG_ALLOW_CROSS=1
    export PKG_CONFIG_SYSROOT_DIR="$sysroot"
    export PKG_CONFIG_LIBDIR="$sysroot/usr/lib/pkgconfig:$sysroot/usr/share/pkgconfig"
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-Wl,-rpath-link,$sysroot/usr/lib"
    cargo +1.96.1 rustc --manifest-path "$client/Cargo.toml" --target-dir "$client/target" \
        --target "$target" --release --locked --offline -- "-C" "link-arg=-Wl,--sysroot=$sysroot"
)
python3 "$project/tools/check-aarch64-deps.py" "$sysroot" "$build/carosign-ui"
aarch64-linux-gnu-readelf -h "$build/carosign-ui" | grep -Eq 'Machine:[[:space:]]+AArch64$'
desktop-file-validate "$client/org.carosign.Ui.desktop"

cp "$project/packaging/arch/ui/PKGBUILD" "$work/PKGBUILD"
cat > "$work/makepkg.conf" <<'CONF'
source /etc/makepkg.conf
CARCH=aarch64
CHOST=aarch64-unknown-linux-gnu
CONF
(
    cd "$work"
    export CAROSIGN_BUILD_DIR="$build" CAROSIGN_PROJECT_DIR="$project"
    export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1}" PKGDEST="$work"
    makepkg --config "$work/makepkg.conf" --nodeps --nosign --noconfirm
)
set -- "$work"/carosign-ui-*-aarch64.pkg.tar.zst
[ "$#" -eq 1 ]
tar -xOf "$1" .PKGINFO | grep -qx 'arch = aarch64'
ln -- "$1" "$output"
echo "Package created: $output"
