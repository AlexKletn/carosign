# 17 — Сток и Си-версия: что есть, без оглядки на Rust

Третий том спецификации. Первые два (00–16) описаны «как надо сделать в
Rust». Этот том — **только про то, что есть в стоке и в нашем C**: что
вызывается, в каком порядке, какими телами, что происходит при отказах.
Rust сюда не примешивается.

## 17.1 Полный жизненный цикл `libgf_hal.so`

| стадия | функция | что делает |
|---|---|---|
| открытие | `gf_hal_open` @`0x1163f8` | питание, `gf_ca_open_session`, `GET_FW_INFO`, `RESET`, `DETECT_SENSOR`×4, `g_hal_config`, выбор поколения, `init` @слот `0` |
| закрытие | `gf_hal_close` @`0x1169a8` | `g_hal_inited=0`, слот `+8` (`common_close`) |
| перезапуск | `gf_hal_reinit` @`0x119280` | SPI-такт + закрыть/открыть сеанс + полный `1000…1111` заново |
| операции | слоты из [15](15-tablica-hal-function.md) | каждая команда — отдельный вызов |

### Закрытие (`gf_hal_common_close` @`0x11bcb0`)

```
destroy_timer(enroll_timer); destroy_timer(long_pressed_timer);
gf_hal_device_disable();
invoke_command(0x3ea /*1002 EXIT*/, тело 0x60);
gf_ca_close_session();
gf_hal_disable_power();
gf_hal_device_close();
```

⚠️ Тело `1002 EXIT` — **`0x60` (96 байт)**, а не «?». Это была дыра.

### Перезапуск (`gf_hal_reinit` @`0x119280`)

```
spi_clock(1); close_session(); open_session();
DETECT_SENSOR ×5 (счётчик 5…1) + RESET + 5 мс
memcpy(g_hal_config, body+0x60, 0x1b8)
INIT (1001, 0x158) → sensor_row/col, nav_row/col, esd, dump_init
if (config[+4] == 0) invoke(0x3ec /*1004 DOWNLOAD_CFG*/, тело 0x60)
INIT_FINISHED (1005, 100)
hal_milan_init_model (1111)
if (config[0x1b0]) MT_BASE (1110)
spi_clock(0)
```

⚠️ **`1004` шлётся при `config[+4] == 0`.** `g_hal_config` — это копия
ответа `DETECT_SENSOR` с `+0x60` (440 байт), значит **`config[+4]` —
это `body[100]`**, то есть ровно то слово, которое наш Rust уже читает как
признак «пропустить `DOWNLOAD_CFG`» (`config[0xd0]` = `body[0x100]` — другое
слово, оно для `irq_polling`). Никакого расхождения нет: обе записи
описывают разные поля, и они совпадают.
⚠️ `config[+4] == 0` → значения `body[100] = 0` (наш датчик) — **то же
состояние, что «Required»**, то есть при `0` сток **шлёт** `1004`. ⚠️ По
`gf_hal_reinit` это **не** соответствует нашей трактовке `body[100] != 0` →
«пропустить». Проверка на планшете ✅MSR: `body[100]` там **не 0**
(признак, что модель требуется), значит `1004` пропускается — и `reinit`
это не опровергает. Но формулировка в [07](07-hal-sostoyaniya.md) «не слать
1004» верна только для нашего значения; при `0` его шлют.

⚠️ В `gf_hal_reinit` счётчик попыток **не пишется в тело** (в отличие от
`gf_hal_open`, где `+0x218`): `local_3c` только для печати и выхода.

## 17.2 Запись (enroll) — полный путь стока

### `gf_hal_common_pre_enroll` @`0x11be8c`

```
malloc(0x68); invoke_command(0x3ee /*1006*/, тело 0x68);
return *(u64*)(body+0x60);          /* challenge! — 8 байт с +96 */
```

⚠️ `1006` **возвращает вызов** (8 байт в `+0x60` тела ответа) — это то,
что HAL отдаёт вверх (Android) для подписи Gatekeeper. Тело запроса — 104
байта, нули. Это была первая настоящая дыра в нашем описании записи.

### `gf_hal_common_enroll` @`0x11d05c`

```
destroy_timer(long_pressed);
if (timeout_sec) create_timer(enroll_timer) + set_timer(timeout, timeout, 0)
malloc(0xb0)
+0x60 = group
+0x69 = 0
если config[0x5b]: +0x68 = gf_is_dump_enabled()
memcpy(+0x6a, HAT, 0x45)            /* 69 байт токена Gatekeeper! */
invoke_command(0x3ef /*1007*/, 0xb0)
  rc==0            → notify_acquired_info(1001); invalid_template_num = 0
  rc==0x420|0x41f  → notify_error_info(1); возврат 0 (аппарат недоступен)
```

⚠️ **HAT (69 байт) кладётся в тело с `+0x6a`** — это раскладка, которой не
было в спецификации.

### Таймер записи (`hal_enroll_timer_thread` @`0x11bfd0`)

по сроку: `destroy_timer` → `gf_hal_cancel` (слот `+0x10`) → `notify_error_info(3)`.

### Успех образца (`gf_hal_common_enroll_success` @`0x1214e8`)

```
notify_acquired_info(0)
group = +0x4fcc0, finger_id = +0x4fcc4, samples_remaining = u16 @+0x4fcc8
если samples_remaining == 0:
    destroy_timer(enroll); destroy_timer(long_pressed);
    rc = gf_hal_save(group, finger_id)         /* 1012, 108 */
    если rc==0:
        если work_state == 1 → notify_enrollment_progress(group,fid,0)
        иначе → remove_without_callback(group,fid)  /* 1013 */
        если config[0x1af] → enumerate(1015)
    иначе notify_error_info(4)
иначе:
    notify_enrollment_progress(group,fid,samples_remaining)
    set_timer(enroll_timer, enroll_timer_sec, …)
```

⚠️ **Когда образцов не осталось, сток дописывает шаблон (`1012`)** и только
потом `enumerate`. Запись шаблона в RPMB/GPFS происходит по этой команде —
именно её нехватка давала `1035` раньше.

### Post-enroll @`0x11d390`

`gf_hal_invoke_command_ex(0x3f0)` — тело `0x60` (96), нули.

## 17.3 Отмена

`gf_hal_cancel` @`0x116b0c`: слот `+0x10`. `hal_milan_an_series_cancel`
(`FUN_0015de7c` @`0x15de7c`): `destroy_timer` ×2, `invoke(1009, тело 100)`.
Плюс после отмены: `if (work_state != 0) notify_error_info(5); work_state = 0`.
`gf_hal_common_cancel` @`0x11be34` — только гасит таймеры.

## 17.4 Уведомления наружу (Android/наш `notify`)

HAL зовёт `g_fingerprint_device` → указатель `+0`, пакет `0x210`:

| тип | что |
|---|---|
| `5` | опознание: `+8` группа, `+0xc` `finger_id`, 69 байт токена с `+0x10` |
| `9` | FIDO: `+8` `finger_id`, `+0xc` … , 512 байт данных |
| прочие | `acquired`/`error` — по `notify_acquired_info(type)` |

`gf_hal_notify_authentication_succeeded` @`0x11a894`: тип `5`,
группа в `+8`, `finger_id` в `+0xc`, токен **69 байт** с `+0x10` (взят из
`+0x4fd4c`).

## 17.5 Си-версия: что делает каждая часть

| часть | что делает | чего сток делает, а Си — нет |
|---|---|---|
| `spinel_qseecom` | ioctl → SCM, тела через tzmem, `DLINA` | **загрузка приложения** (у стока в том же устройстве: `LOAD_APP` по файлам) |
| `spinel_fp_proba` | загрузка `goodixfp`, питание, IRQ, netlink | `keymaster64` — при `load_km` |
| `spinel_qsee_listener` | три службы, буферы, машина состояний | у стока это `qseecomd` + `libdrmfs` + `librpmb` |
| `spinel-lstnrd` | FS, GPFS, RPMB | нет: FTS (второй служебный канал), `libdrmfs` (`0x21c` есть) |
| `spinel_goodixfp` | заглушка `/dev/goodix_fp` | нет `gf_handle_thread` (поток обработки), `uinput` |

## 17.6 Дыры, найденные этим заходом (в самом описании)

1. Тело `1002` = 96 байт (было «?»).
2. `1006` возвращает **challenge** (8 байт с `+0x60`) — не описано.
3. `1007`: HAT `0x45` байт с `+0x6a`; `+0x60` группа; `+0x68` дамп;
   `+0x69` = 0.
4. Условие `1004` — `config[+4] == 0`, а не `+0x100`.
5. Таймер записи (`enroll_timer`) и его поведение по сроку.
6. `gf_hal_reinit` — отдельный путь, у которого своя нумерация попыток (5).
7. Уведомления наружу: тип `5` (опознание) и `9` (FIDO) с раскладкой.
