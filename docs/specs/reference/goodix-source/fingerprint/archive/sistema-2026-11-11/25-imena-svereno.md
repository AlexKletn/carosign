# 25 — Имена режимов, операций, навигации, ошибок и команд: ПРОЧИТАНО ✅BIN

Все пять таблиц прочитаны из образа (`libgf_hal.so`, `.data` + `.rodata`)
с поправкой: указатель в записи — **файловое смещение**, адрес в образе
Ghidra = значение `+ 0x10000`. Записи по **16 байт** (`u32 код`,
`u32 зарезерв`, `u64 указатель`).

## 25.1 Операции (`operation_table` @`0x4177c8`)

| код | имя |
|---|---|
| `0` | `OPERATION_ENROLL` |
| `1` | `OPERATION_AUTHENTICATE_IMAGE` |
| `2` | `OPERATION_AUTHENTICATE_FF` |
| `3` | `OPERATION_AUTHENTICATE_SLEEP` |
| `4` | `OPERATION_AUTHENTICATE_FIDO` |
| `5` | `OPERATION_FINGER_BASE` |
| `6` | `OPERATION_NAV` |
| `7` | `OPERATION_NAV_BASE` |
| `8` | `OPERATION_CHECK_FINGER_LONG_PRESS` |
| `9` | `OPERATION_HOME_KEY` |
| `10` | `OPERATION_POWER_KEY` |
| `11` | `OPERATION_CAMERA_KEY` |
| `12` | `OPERATION_HEARTBEAT_KEY` |
| `13…21` | `OPERATION_TEST_IMAGE_MODE`, `_DEBUG_MODE`, `_FF_MODE`, `_KEY_MODE`, `_NAV_MODE`, `_NAV_BASE_MODE`, `_FINGER_BASE_MODE`, `_IDLE_MODE`, `_SLEEP_MODE` |
| `22…23` | `_HBD_DEBUG_MODE`, `_HBD_MODE` |
| `24`, `24`, `25`, `26` | `_PIXEL_SHORT_STREAK`, `_PIXEL_OPEN_STEP1`, `_PIXEL_OPEN_STEP2`, `_PIXEL_OPEN_STEP3` |
| `27…28` | `_BAD_POINT_RECODE_BASE`, `_BAD_POINT` |
| `29…31` | `_PERFORMANCE`, `_SPI_PERFORMANCE`, `_SPI_TRANSFER` |
| `32…34` | `_FRR_FAR_RECORD_CALIBRATION`, `_ENROLL`, `_AUTHENTICATE` |
| `35…36` | `_UNTRUSTED_ENROLL`, `_UNTRUSTED_AUTHENTICATE` |
| `37…38` | `_CHECK_FINGER_EVENT`, `_BIO_CALIBRATION` |

⚠️ **Код `24` стоит дважды** (`_PIXEL_SHORT_STREAK` и
`_PIXEL_OPEN_STEP1`) — так записано в самой таблице, это не ошибка чтения.

## 25.2 Режимы (`mode_table` @`0x416fa8`)

| код | имя |
|---|---|
| `0` | `MODE_IMAGE` |
| `1` | `MODE_KEY` |
| `2` | `MODE_SLEEP` |
| `3` | `MODE_FF` |
| `4` | `MODE_NAV` |
| `5` | `MODE_NAV_BASE` |
| `6` | `MODE_DEBUG` |
| `7` | `MODE_FINGER_BASE` |
| `8` | `MODE_IDLE` |
| `9` | `MODE_HBD` |
| `10` | `MODE_HBD_DEBUG` |
| `11` | `MODE_IMAGE_CONTINUE` |
| `12…15` | `MODE_TEST_BAD_POINT`, `_PIXEL_OPEN_DEFAULT`, `_PIXEL_OPEN_POSITIVE`, `_BAD_POINT_FINGER_BASE` |
| `17` | `MODE_MAX` |
| `1000` | `INVALID_MODE` |

⚠️ Кода `16` в таблице нет (пропуск), `17` — `MODE_MAX`, `1000` не
существует, а `INVALID_MODE` (`@0xae420`) лежит **за** концом массива
`MODE_MAX`, то есть `gf_strmode` для неизвестного кода ищет до
`MODE_MAX=17` и подставляет следующий узел. Это не «режим 16», а дыра.

## 25.3 Навигация (`navmode_table` @`0x416808`)

| код | имя |
|---|---|
| `0` | `GF_NAV_MODE_NONE` |
| `1` | `GF_NAV_MODE_X` |
| `2` | `GF_NAV_MODE_Y` |
| `4` | `GF_NAV_MODE_Z` |
| `3` | `GF_NAV_MODE_XY` |
| `5` | `GF_NAV_MODE_XZ` |
| `6` | `GF_NAV_MODE_YZ` |
| `7` | `GF_NAV_MODE_XYZ` |
| `8` | `GF_NAV_MODE_MAX` |

⚠️ **Порядок задан нарочно**: `Z` идёт раньше `XY` — поэтому прямой
поиск по индексу (`nav_mode < 8`) у `gf_hal_common_navigate` **не
совпадает** с порядком записи. Это правило, а не случайность.

## 25.4 Ошибки (`err_table` @`0x416078`) — начало

`0` → `GF_SUCCESS`; `1000` → `GF_ERROR_BASE`; `1001` → `OUT_OF_MEMORY`;
`1002` → `OPEN_TA_FAILED`; `1003` → `BAD_PARAMS`; `1004` → `NO_SPACE`;
`1005` → `REACH_FINGERS_UPLIMIT`; `1006` → `NOT_MATCH`; `1007` →
`CANCELED`; `1008` → `TIMEOUT`; `1009` → `PREPROCESS_FAILED`; `1010` →
`GENERIC`.

⚠️ **Это и есть то, что сток возвращает вместо `errno`**: `1006` — это
`GF_ERROR_NOT_MATCH` (несовпадение), а `1010` — `GF_ERROR_GENERIC`.
Значит «ошибка канала» и «нет совпадения» в ответе — это один словарь.

## 25.5 Команды (`cmd_id_table` @`0x4170c8`)

`1000 DETECT_SENSOR`, `1001 INIT`, `1002 EXIT`, `1003 DOWNLOAD_FW`,
`1004 DOWNLOAD_CFG`, `1005 INIT_FINISHED`, `1006 PRE_ENROLL`,
`1007 ENROLL`, `1008 POST_ENROLL`, `1009 CANCEL`, `1010 AUTHENTICATE`,
`1011 GET_AUTH_ID`, … — **совпадает с `PERECHNI.txt` побайтно по смыслу**
и с тем, что мы уже использовали в спецификации.

## 25.6 Что это дало

* Больше **не нужно** называть операции числами: имена есть.
* `OPERATION_NONE` в таблице отсутствует — `52` в журнале не является
  кодом таблицы.
* Ошибка `1006` из документации HAL — это `NOT_MATCH`, а `1010` —
  `GENERIC`; теперь «что значит код» берётся из этой таблицы, а не
  пересказывается.
