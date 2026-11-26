#!/bin/sh
# Creates a local source snapshot and a PKGBUILD with its checksum.
set -eu
if [ "$#" -ne 1 ]; then
    echo "Использование: sh $0 /путь/к/новому-каталогу" >&2
    exit 2
fi
case "$1" in /*) output=$1 ;; *) echo 'Нужен абсолютный путь' >&2; exit 2 ;; esac
if [ -e "$output" ] || [ -L "$output" ]; then
    echo 'Каталог уже существует' >&2
    exit 1
fi
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -m 755 -- "$output"
archive="$output/carosign-0.1.0.tar.gz"
tar --sort=name --format=ustar --mtime='@0' --owner=0 --group=0 --numeric-owner \
    --exclude=target --exclude=__pycache__ \
    --transform='s,^,carosign-0.1.0/,' -C "$root" \
    -cf - Cargo.toml Cargo.lock README.md crates contracts compat packaging docs tools locales | gzip -n > "$archive"
digest=$(sha256sum "$archive")
digest=${digest%% *}
sed "s/sha256sums=('SKIP')/sha256sums=('$digest')/" \
    "$root/packaging/arch/PKGBUILD.source" > "$output/PKGBUILD"
echo "Создан исходный снимок с PKGBUILD: $output"
