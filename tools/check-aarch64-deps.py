#!/usr/bin/env python3
"""Проверка ELF-зависимостей собранных файлов по отдельному дереву системы.

Это статическая проверка: она не запускает целевой загрузчик и не доказывает
работоспособность программы, ABI ядра или подлинность дерева.
"""

import argparse
import os
import re
import subprocess
import sys
from collections import deque
from pathlib import Path


def readelf(*args):
    return subprocess.run(
        ["aarch64-linux-gnu-readelf", *map(str, args)],
        check=True, capture_output=True, text=True,
    ).stdout


def resolve_in_root(root, path):
    """Разрешить ссылки как внутри chroot, не выходя в систему хозяина."""
    parts = deque(Path(path).parts)
    current = []
    links = 0
    while parts:
        part = parts.popleft()
        if part in ("/", "", "."):
            continue
        if part == "..":
            if current:
                current.pop()
            continue
        candidate = root.joinpath(*current, part)
        if candidate.is_symlink():
            links += 1
            if links > 40:
                raise ValueError(f"слишком много ссылок: {path}")
            target = os.readlink(candidate)
            if target.startswith("/"):
                current.clear()
            parts.extendleft(reversed(Path(target).parts))
        else:
            current.append(part)
    return root.joinpath(*current)


def needed_versions(path):
    output = readelf("-V", path)
    needs = output.partition("Version needs section")[2]
    needs = needs.split("Version definition section", 1)[0]
    result = {}
    lib = None
    for line in needs.splitlines():
        m = re.search(r"\bFile: (\S+)", line)
        if m:
            lib = m.group(1)
            result.setdefault(lib, set())
        m = re.search(r"\bName: (\S+)", line)
        if m and lib:
            result[lib].add(m.group(1))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rootfs", type=Path, help="путь к целевому дереву")
    parser.add_argument("elf", nargs="+", type=Path, help="проверяемые ELF")
    args = parser.parse_args()
    root = args.rootfs.resolve(strict=True)
    if not root.is_dir():
        parser.error("rootfs должен быть каталогом")
    search = ("/lib", "/usr/lib", "/lib/aarch64-linux-gnu", "/usr/lib/aarch64-linux-gnu")
    queue = deque(args.elf)
    visited = set()
    errors = []
    while queue:
        path = queue.popleft()
        if path in visited:
            continue
        visited.add(path)
        header = readelf("-h", path)
        if not re.search(r"Machine:\s+AArch64\s*$", header, re.MULTILINE):
            errors.append(f"{path}: не AArch64")
            continue
        program_headers = readelf("-l", path)
        interp = re.search(r"Requesting program interpreter: ([^\]]+)", program_headers)
        if interp:
            loader = resolve_in_root(root, interp.group(1))
            if not loader.is_file():
                errors.append(f"{path}: отсутствует загрузчик {interp.group(1)}")
            else:
                queue.append(loader)
        dynamic = readelf("-d", path)
        runpath = re.search(r"\(RUNPATH\).*?\[(.*?)\]", dynamic)
        directories = tuple(runpath.group(1).split(":")) if runpath else ()
        if re.search(r"\(RPATH\)", dynamic) or any(
            not directory.startswith("/") or "$" in directory for directory in directories
        ):
            errors.append(f"{path}: неподдерживаемый RPATH/RUNPATH")
            continue
        names = re.findall(r"\(NEEDED\).*?\[(.*?)\]", dynamic)
        versions = needed_versions(path)
        for name in versions.keys() - set(names):
            errors.append(f"{path}: версия относится к отсутствующей зависимости {name}")
        for name in names:
            if "/" in name or name in (".", ".."):
                errors.append(f"{path}: недопустимый DT_NEEDED {name}")
                continue
            candidates = [resolve_in_root(root, f"{directory}/{name}") for directory in (*directories, *search)]
            lib = next((candidate for candidate in candidates if candidate.is_file()), None)
            if lib is None:
                errors.append(f"{path}: отсутствует {name}")
                continue
            definitions = readelf("-V", lib).partition("Version definition section")[2]
            definitions = definitions.split("Version needs section", 1)[0]
            offered = set(re.findall(r"\bName: (\S+)", definitions))
            missing = versions.get(name, set()) - offered
            if missing:
                errors.append(f"{path}: {name} не предоставляет {', '.join(sorted(missing))}")
            queue.append(lib)
            print(f"{path.name}: {name} → {lib.relative_to(root)}")
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("Статические ELF-зависимости найдены; запуск и полная совместимость не проверялись.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Не удалось проверить зависимости: {error}", file=sys.stderr)
        sys.exit(2)
