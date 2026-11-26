# Review sessions: stock and C version (10–11 November 2026)

Хронология. Каждый заход — со своей ошибкой и своей находкой; полезно
читать, когда кажется, что «всё уже перебрано».

## 10 ноября

* Разбор `libgf_hal.so` точечно (`objdump`), подъём Ghidra headless,
  выгрузка декомпиляции (`tools/ghidra/DekompChasty.java`).
* Найдено: `irq_pre_process` обнуляет тело; повторы только на ступенях
  `4`/`5`; `1012`/`1085` после `1086`.

## 11 ноября

* [Проверка 1](check-1-event-and-save.md) — событие, `1012`/`1085`.
* [Проверка 2](check-2-commands-and-bodies.md) — тела и отправители команд.
* [Проверка 3](check-3-power-ports.md) — питание, повторы, порт netlink.
* [Проверка 4](check-4-without-rust.md) — описание без Rust.
* [Проверка 5](check-5-navigation-uinput-model.md) — навигация, `uinput`, модель.
* [Проверка 6](check-6-table-names.md) — имена таблиц прочитаны.
* [Проверка 7](check-7-stock-kernel-driver.md) — драйвер ядра стока (по ROM).
* [Проверка 8](check-8-chip-and-weights.md) — чип и выбор весов.
* [Веса: ход поиска](02-model-weights-search.md), [таблицы переходов](03-jump-tables-first-try.md),
  [адреса файл/образ](04-file-and-image-addresses.md).

## Отдельно

* [Измерения события](10-event-measurements.md) — замеры 10 ноября.
* [Разбор вечера](11-evening-analysis.md), [состояние](12-state-10-november.md).
