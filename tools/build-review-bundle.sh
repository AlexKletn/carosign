#!/bin/sh
# Builds the review bundle.
set -eu

if [ "$#" -ne 1 ] && [ "$#" -ne 3 ]; then
    echo "Использование: $0 /абсолютный/путь/к/новому-архиву.tar [--target тройка]" >&2
    exit 2
fi
case "$1" in
    /*) output=$1 ;;
    *) echo 'Путь к архиву должен быть абсолютным' >&2; exit 2 ;;
esac
target=
if [ "$#" -eq 3 ]; then
    if [ "$2" != --target ]; then
        echo 'Ожидался --target' >&2
        exit 2
    fi
    case "$3" in
        ''|[!A-Za-z0-9]*|*[!A-Za-z0-9_.-]*) echo 'Недопустимая тройка target' >&2; exit 2 ;;
        *) target=$3 ;;
    esac
fi
if [ -e "$output" ] || [ -L "$output" ]; then
    echo 'Архив уже существует' >&2
    exit 1
fi

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
parent=$(dirname -- "$output")
test -d "$parent"
stage=$(mktemp -d "${TMPDIR:-/tmp}/carosign-review.XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT HUP INT TERM

cd "$root"
# The offline build requires Cargo.lock.
if [ -n "$target" ]; then
    cargo build --locked --offline --release --target "$target" \
        -p carosignd -p carosign-broker -p carosignctl \
        -p carosign-fprintd-compat -p pam-carosign
    build_dir="target/$target/release"
else
    cargo build --locked --offline --release \
        -p carosignd -p carosign-broker -p carosignctl \
        -p carosign-fprintd-compat -p pam-carosign
    build_dir=target/release
fi

mkdir -p "$stage/bundle/bin" "$stage/bundle/lib" "$stage/bundle/docs"
install -m 0755 "$build_dir/carosignd" "$stage/bundle/bin/carosignd"
install -m 0755 "$build_dir/carosign-broker" "$stage/bundle/bin/carosign-broker"
install -m 0755 "$build_dir/carosignctl" "$stage/bundle/bin/carosignctl"
install -m 0755 "$build_dir/carosign-config" "$stage/bundle/bin/carosign-config"
install -m 0755 "$build_dir/carosign-fprintd-compat" "$stage/bundle/bin/carosign-fprintd-compat"
install -m 0755 "$build_dir/libpam_carosign.so" "$stage/bundle/lib/pam_carosign.so"
install -m 0644 docs/guides/REVIEW_BUNDLE.md "$stage/bundle/docs/README.md"
install -m 0644 docs/contracts/FPRINTD_WIRE_BASELINE.md "$stage/bundle/docs/FPRINTD_WIRE_BASELINE.md"
(
    cd "$stage/bundle"
    sha256sum bin/carosignd bin/carosign-broker bin/carosignctl bin/carosign-config \
        bin/carosign-fprintd-compat lib/pam_carosign.so \
        docs/README.md docs/FPRINTD_WIRE_BASELINE.md > SHA256SUMS
)

# Stable ordering, ownership, and timestamps make the archive reproducible for identical inputs.
# Compiled bytes can still vary with the toolchain and build environment.
tar --sort=name --format=ustar --mtime='@0' --owner=0 --group=0 --numeric-owner \
    -cf "$stage/review.tar" -C "$stage/bundle" .
# The hard link cannot replace an existing destination.
# Keep the temporary file on the same filesystem as the destination.
temporary=$(mktemp "$parent/.carosign-review.XXXXXXXX")
trap 'rm -rf -- "$stage"; rm -f -- "$temporary"' EXIT HUP INT TERM
cat "$stage/review.tar" > "$temporary"
ln "$temporary" "$output"
rm -f -- "$temporary"
echo "Создан набор для проверки: $output"
