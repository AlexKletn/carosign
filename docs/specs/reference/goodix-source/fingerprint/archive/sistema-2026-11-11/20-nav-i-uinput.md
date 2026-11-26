# 20 — Сток: навигация и `uinput` целиком

Разбор 11 ноября. Закрывает пункты 2 и 3 из «чего нет» в
[PROVERKA-SPEC-4](../PROVERKA-SPEC-4.md).

## 20.1 Устройство `uinput` ✅BIN

`gf_hal_uinput_device_open` @`0x136b50`:

```
open("/dev/uinput", 0x801 /* O_WRONLY|O_NONBLOCK */)
ioctl(UI_SET_EVBIT 0x40045564, 1 /* EV_KEY */)
для 7 клавиш: ioctl(UI_SET_KEYBIT 0x40045565, код)
memcpy структуры 0x45c (1116) байт:
    имя "uinput_nav"
    +0x450..0x456: u16 3, 1, 1, 1        /* id.bustype=3(BUS_USB?), vendor, product, version */
write(fd, uidev, 0x45c)
ioctl(UI_DEV_CREATE 0x5501)
```

При любом отказе — `UI_DEV_DESTROY` (`0x5502`), `close`, код `0x3f6`.

### Клавиши ✅BIN (`.data` @`0xd1038`, 7 значений `u16`)

| код | Linux | смысл |
|---|---|---|
| `0x6c` | `KEY_DOWN` | «вниз» |
| `0x67` | `KEY_UP` | «вверх» |
| `0x6a` | `KEY_RIGHT` | «вправо» |
| `0x69` | `KEY_LEFT` | «влево» |
| `0x72` | `KEY_MENU` | «меню» |
| `0xd9` | `KEY_BACK` | «назад» |
| `0x73` | `KEY_KPENTER` | «enter» |

⚠️ Четыре стрелки, меню, назад, enter — ровно то, что умеет жест по
датчику. Этим `uinput` и заканчивается.

### Отправка ✅BIN

`gf_hal_uinpt_reportkey` @`0x1370b0`: `input_event` **24 байта** (aarch64):
`+0` время (нули), `+0x10` тип, `+0x12` код, `+0x14` значение; `write`.
Код отказа — `0x3f9`.

`gf_hal_send_uinput_nav_event` @`0x1371f4`: печатает `nav=%s, nav_code=%d`,
затем отправляет нажатие/отпускание соответствующей клавиши; без устройства
— `0x3f8`.

## 20.2 Команды навигации ✅BIN

| команда | тело | условие | источник |
|---|---|---|---|
| `1071 NAVIGATE` | 100, `+0x60` = `nav_mode` | `nav_mode < 8`, иначе `0x3eb` | `gf_hal_common_navigate` @`0x11e9b4` |
| `1072 DETECT_NAV_EVENT` | 104 | IRQ выключен на время вызова | `FUN_0015c084` @`0x15c084` |
| `1073 NAVIGATE_COMPLETE` | 96, нули | — | `gf_hal_nav_complete` @`0x11a4b4` |

Ответ `1072`: `+0x60` — код события. Если он не `0`, не `7`, не `8` и
`g_nav_click_status ≠ 0` — зовётся `gf_hal_nav_listener(code)`.

## 20.3 Режимы навигации ✅BIN

Таблица `navmode_table` @`0x416808` ✅BIN:

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

`gf_hal_common_navigate` принимает **`< 8`**, то есть `8`
(`GF_NAV_MODE_MAX`) командой не поддерживается.

## 20.4 Интервалы навигации ✅BIN

`gf_hal_nav_assert_config_interval` @`0x11a5e8` правит `g_hal_config`:

* `config[0x3c]` (нажатие), если вне `0x50…1000` — ставится **`300`**;
* `config[0x40]` (отпускание?), если вне `600…2000` — ставится **`700`**.

⚠️ Это значения по умолчанию, а не паспортные: датчик их отдаёт в ответе
`1001` (`config` = ответ с `+0x60`), а HAL лишь ограничивает разумными
пределами. Если бы их не было — жест «дребезжал» бы.

## 20.5 Коды события навигации в ответе `1072`

Числа из кода `FUN_0015c084`: `0` — нет события, `7`/`8` — служебные
(исключения), остальные — жест.

`gf_hal_nav_code_convert` @`0x1199f0` переводит код жеста с учётом
поворота — см. [21](21-povorot-i-sluzhebnye.md) §21.1.
