#!/bin/sh
# Stages a root filesystem tree for layout inspection.
set -eu

if [ "$#" -ne 2 ]; then
    echo "Использование: sh $0 /путь/к/пустому-каталогу target-тройка" >&2
    exit 2
fi
case "$1" in
    /*) stage=$1 ;;
    *) echo 'Нужен абсолютный путь к каталогу' >&2; exit 2 ;;
esac
case "$2" in
    ''|[!A-Za-z0-9]*|*[!A-Za-z0-9_.-]*) echo 'Недопустимая тройка target' >&2; exit 2 ;;
    *) target=$2 ;;
esac
if [ ! -d "$stage" ] || [ -L "$stage" ] || [ "$(CDPATH= cd -- "$stage" && pwd -P)" = / ]; then
    echo 'Нужен существующий пустой каталог, отличный от корня' >&2
    exit 1
fi
if [ -n "$(find "$stage" -mindepth 1 -maxdepth 1 -print -quit)" ]; then
    echo 'Каталог должен быть пустым' >&2
    exit 1
fi

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
build_dir="$root/target/$target/release"
for name in carosignd carosign-broker carosignctl carosign-fprintd-compat libpam_carosign.so; do
    if [ ! -f "$build_dir/$name" ]; then
        echo "Нет результата сборки: $build_dir/$name" >&2
        exit 1
    fi
done

mkdir -p "$stage/usr/bin" "$stage/usr/lib/security" "$stage/usr/share/doc/carosign-review"
for name in carosignd carosign-broker carosignctl carosign-fprintd-compat; do
    install -m 0755 "$build_dir/$name" "$stage/usr/bin/$name"
done
install -m 0755 "$build_dir/libpam_carosign.so" "$stage/usr/lib/security/pam_carosign.so"
install -m 0644 "$root/docs/guides/REVIEW_BUNDLE.md" "$stage/usr/share/doc/carosign-review/README.md"
install -m 0644 "$root/docs/contracts/FPRINTD_WIRE_BASELINE.md" "$stage/usr/share/doc/carosign-review/FPRINTD_WIRE_BASELINE.md"
(
    cd "$stage"
    sha256sum usr/bin/carosignd usr/bin/carosign-broker usr/bin/carosignctl \
        usr/bin/carosign-fprintd-compat usr/lib/security/pam_carosign.so \
        usr/share/doc/carosign-review/README.md \
        usr/share/doc/carosign-review/FPRINTD_WIRE_BASELINE.md \
        > SHA256SUMS
)
echo 'Подготовлено дерево для проверки; это не установочный пакет и не готовая служба.'
