#!/bin/sh
# Compares two builds with the same environment and toolchain.
set -eu
if [ "$#" -ne 0 ]; then
    echo "Использование: sh $0" >&2
    exit 2
fi
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/carosign-arch-compare.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT HUP INT TERM
sh "$root/tools/build-arch.sh" "$work/first.pkg.tar.zst"
sh "$root/tools/build-arch.sh" "$work/second.pkg.tar.zst"
if ! cmp -s "$work/first.pkg.tar.zst" "$work/second.pkg.tar.zst"; then
    echo 'Пакеты различаются: исходники и окружение должны оставаться неизменными.' >&2
    sha256sum "$work/first.pkg.tar.zst" "$work/second.pkg.tar.zst" >&2
    exit 1
fi
printf 'Два пакета совпадают в текущем окружении: '
sha256sum "$work/first.pkg.tar.zst" | cut -d ' ' -f 1
