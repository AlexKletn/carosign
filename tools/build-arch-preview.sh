#!/bin/sh
# Builds an Arch Linux ARM preview package on the host without installing it or configuring PAM.
set -eu
if [ "$#" -ne 1 ] && [ "$#" -ne 3 ]; then
    echo "Использование: sh $0 /абсолютный/путь/к/новому-пакету.pkg.tar.zst [--check-rootfs /путь/к/rootfs]" >&2
    exit 2
fi
case "$1" in /*) output=$1 ;; *) echo 'Нужен абсолютный путь' >&2; exit 2 ;; esac
rootfs=
if [ "$#" -eq 3 ]; then
    [ "$2" = --check-rootfs ] || { echo 'Неизвестный параметр' >&2; exit 2; }
    case "$3" in /*) rootfs=$3 ;; *) echo 'Для rootfs нужен абсолютный путь' >&2; exit 2 ;; esac
    [ -d "$rootfs" ] || { echo 'Каталог rootfs не существует' >&2; exit 2; }
fi
if [ -e "$output" ] || [ -L "$output" ]; then
    echo 'Файл уже существует' >&2
    exit 1
fi
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
parent=$(dirname -- "$output")
test -d "$parent"
command -v makepkg >/dev/null
command -v aarch64-linux-gnu-readelf >/dev/null
# makepkg records builddir and startdir in .BUILDINFO. A fixed path keeps
# metadata reproducible. Refuse to overwrite an existing build directory.
work="/tmp/carosign-arch-preview-build-$(id -u)"
if ! mkdir -m 700 -- "$work"; then
    echo "Рабочий каталог уже занят: $work" >&2
    exit 1
fi
temporary=
trap 'rm -rf -- "$work"; if [ -n "$temporary" ]; then rm -f -- "$temporary"; fi' EXIT HUP INT TERM

sh "$root/tools/build-review-aarch64.sh" "$work/review.tar"
build="$root/target/aarch64-unknown-linux-gnu/release"
for name in carosignd carosign-broker carosignctl carosign-fprintd-compat libpam_carosign.so; do
    aarch64-linux-gnu-readelf -h "$build/$name" | grep -Eq 'Machine:[[:space:]]+AArch64$' || {
        echo "Неверная архитектура: $name" >&2
        exit 1
    }
done

# Optionally checks ELF dependencies before publishing the package.
# This check does not execute target code or establish kernel ABI compatibility or rootfs trust.
if [ -n "$rootfs" ]; then
    python3 "$root/tools/check-aarch64-deps.py" "$rootfs" \
        "$build/carosignd" "$build/carosign-broker" "$build/carosignctl" \
        "$build/carosign-fprintd-compat" "$build/libpam_carosign.so"
fi

# Without CARCH, makepkg labels the package with the host architecture even with --ignorearch.
cp "$root/packaging/arch-preview/PKGBUILD" "$work/PKGBUILD"
cat > "$work/makepkg.conf" <<'EOF'
source /etc/makepkg.conf
CARCH=aarch64
CHOST=aarch64-unknown-linux-gnu
EOF
(
    cd "$work"
    export CAROSIGN_PREVIEW_BUILD_DIR="$build"
    export CAROSIGN_PREVIEW_PROJECT_DIR="$root"
    export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1}"
    export PKGDEST="$work"
    makepkg --config "$work/makepkg.conf" --nodeps --nosign --noconfirm
)
package="$work/carosign-preview-0.0.0-1-aarch64.pkg.tar.zst"
test -f "$package"
tar -xOf "$package" .PKGINFO | grep -qx 'arch = aarch64'
temporary=$(mktemp "$parent/.carosign-arch-preview.XXXXXXXX")
cat "$package" > "$temporary"
ln "$temporary" "$output"
rm -f -- "$temporary"
temporary=
echo "Создан дорелизный проверочный пакет: $output"
