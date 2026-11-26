#!/bin/sh
# Runs package installation and removal with pacman in a disposable root.
# Target libraries and executable code are outside this test’s scope.
set -eu

if [ "$#" -ne 1 ]; then
    printf 'использование: %s /путь/к/пакету.pkg.tar.zst\n' "$0" >&2
    exit 2
fi
package=$(realpath -- "$1")
[ -f "$package" ] || { printf 'пакет не найден\n' >&2; exit 1; }

exec unshare --user --map-root-user --mount sh -s -- "$package" <<'SCRIPT'
set -eu
work=$(mktemp -d /tmp/carosign-pacman-test.XXXXXXXX)
trap 'rm -rf "$work"' EXIT HUP INT TERM
root=$work/root
mkdir -p "$root/var/lib/pacman" "$work/cache" "$work/hooks"
printf '[options]\nArchitecture = aarch64\nSigLevel = Never\n' > "$work/pacman.conf"
pacman_cmd() {
    pacman --config "$work/pacman.conf" --root "$root" \
        --dbpath "$root/var/lib/pacman" --cachedir "$work/cache" \
        --hookdir "$work/hooks" --noconfirm "$@"
}

# Model installed dependencies because the disposable root starts empty.
# Signature checks are disabled only for the local fixture.
pacman_cmd --assume-installed glibc --assume-installed libgcc \
    --assume-installed dbus --assume-installed pam --assume-installed systemd \
    -U "$1"
pacman_cmd -Q carosign
pacman_cmd -Qkk carosign

config=$root/etc/carosign/carosignd.conf
test -f "$config"
printf '\n# изменение администратора для проверки pacman\n' >> "$config"
pacman_cmd --assume-installed glibc --assume-installed libgcc \
    --assume-installed dbus --assume-installed pam --assume-installed systemd \
    -U "$1"
grep -q 'изменение администратора для проверки pacman' "$config"
pacman_cmd -R carosign
test ! -e "$root/usr/bin/carosignd"
test -f "$config.pacsave"
grep -q 'изменение администратора для проверки pacman' "$config.pacsave"
printf 'Изолированная установка, повторная установка и удаление прошли; изменённая конфигурация сохранена.\n'
SCRIPT
