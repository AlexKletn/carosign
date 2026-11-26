# 24 — Именованные таблицы-строки отпечатка: где лежат и как прочитать



## 24.1 Где таблицы

✅BIN `readelf -rW libgf_hal.so` (раздел `.rela.dyn`):

| переменная-указатель | адрес | цель |
|---|---|---|
| `PTR_mode_table` | `0xd0d48` | `0x416fa8` |
| `PTR_operation_table` | `0xd0e90` | `0x4177c8` |
| `PTR_navmode_table` | `0xd0ee8` | `0x416808` |
| `PTR_err_table` | `0xd0e08` | `0x416078` |
| `PTR_cmd_id_table` | `0xd0ec8` | `0x4170c8` |

Таблица — массив записей **16 байт**: `u32 код`, `u32 выравнивание`,
`u64 указатель на строку`.

## 24.2 Почему строки «пустые»

⚠️ Указатели в таблице — с нулевыми **адресами**, пока их не разрешит
загрузчик: читая `.data` прямо из файла, получаешь нули или мусор. Указатели
на строки лежат не как готовый адрес, а как **смещение**, которое ОС
прибавит к базе при загрузке.

✅ Поэтому читаем **по файлу** с поправкой: `адрес = значение + 0x10000`
(база образа в декомпиляции Ghidra). Так и получены списки в 24.4.

## 24.4 Что должно получиться

По строкам в `.rodata` ✅BIN известны имена (ещё до таблиц):

### Режимы (`MODE_*`), @`0xae2d0…0xae420`

```
MODE_IMAGE   MODE_KEY     MODE_SLEEP   MODE_FF      MODE_NAV
MODE_NAV_BASE  MODE_DEBUG  MODE_FINGER_BASE  MODE_IDLE  MODE_HBD
MODE_HBD_DEBUG MODE_IMAGE_CONTINUE  MODE_TEST_BAD_POINT
MODE_TEST_PIXEL_OPEN_DEFAULT  MODE_TEST_PIXEL_OPEN_POSITIVE
MODE_TEST_BAD_POINT_FINGER_BASE  MODE_MAX  INVALID_MODE
```
18 имён — ровно 18 записей в `mode_table` (последние две — `MODE_MAX`,
`INVALID_MODE`). ⚠️ Сопоставление «имя ↔ номер» даст только вызов.

### Операции (`OPERATION_*`), @`0xaf0a0…0xaf2e8`

```
OPERATION_ENROLL  OPERATION_AUTHENTICATE_IMAGE  OPERATION_AUTHENTICATE_FF
OPERATION_AUTHENTICATE_SLEEP  OPERATION_AUTHENTICATE_FIDO
OPERATION_FINGER_BASE  OPERATION_NAV  OPERATION_NAV_BASE
OPERATION_CHECK_FINGER_LONG_PRESS  OPERATION_HOME_KEY  OPERATION_POWER_KEY
OPERATION_CAMERA_KEY  OPERATION_HEARTBEAT_KEY
OPERATION_TEST_IMAGE_MODE … OPERATION_TEST_SLEEP_MODE
```
Остальные `OPERATION_TEST_*` встречаются в строках журнала — их имена
**не** попадают в таблицу.

### Команды — таблица `cmd_id_table` @`0x4170c8`

Коды по `1000, 1001, …` — та же последовательность, что в `PERECHNI.txt`,
но строки в образе не размечены.

## 24.5 Сверка с уже известным

* ✅ `gf_strmode(1)` должно дать `MODE_IMAGE`? — **нет**, это надо
  доказать вызовом; в `PERECHNI`/журналах `MODE_IMAGE` встречается, но
  номер 1 не подтверждён.
* ✅ Для `gf_stroperation` журнал подтверждает: `g_operation=1` →
  `OPERATION_AUTHENTICATE_IMAGE`, `g_operation=52` → `OPERATION_NONE`
  (печать `[GF_HAL][gf_hal_milan]` в замере). Но `OPERATION_NONE` в
  `operation_table` **отсутствует** — значит `52` — не код таблицы, а
  «не найдено» (по коду возвращается `0x11`, последняя запись).

## 24.6 Итог

✅ Все пять таблиц **прочитаны и сопоставлены** — см.
[25](25-imena-svereno.md). Указатели в таблице — **файловые смещения**,
адрес = значение `+ 0x10000`. Без этой поправки читается мусор — на этом
и встал первый заход.
