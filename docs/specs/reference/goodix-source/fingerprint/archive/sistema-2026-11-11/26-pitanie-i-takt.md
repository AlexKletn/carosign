# 26 — Сток: питание, сброс и тактирование (по двоичному)

Закрывает последний из трёх пунктов «не разобрано» (запрос
`gf_hal_enable_power`/`gf_hal_disable_power` и порядок).

## 26.1 Где живёт

HAL **сам железо не трогает**: у него есть только ioctl'ы к драйверу
`/dev/goodix_fp` (магия `'g'`), и всё питание/сброс/такт — это ioctl'ы.
Значит «порядок питания стока» = **порядок ioctl'ов**.

## 26.2 Порядок при открытии ✅BIN (`gf_hal_open` @`0x1163f8`)

```
gf_hal_device_open()      → open("/dev/goodix_fp"), sem_init, uinput
gf_ca_open_session()      → /dev/qseecom, LOAD_APP
gf_hal_enable_power()     → ioctl GF_IOC_ENABLE_POWER (0x6707)
gf_hal_get_fw_info(&b)    → ioctl GF_IOC_GET_FW_INFO (0x8001670b)
if (b == 0) gf_hal_reset_chip()   → ioctl GF_IOC_RESET (0x6702)
цикл DETECT_SENSOR (4 попытки):
    gf_hal_control_spi_clock(1)   → ioctl ENABLE_SPI_CLK (0x40046705,
                                     аргумент = g_spi_speed)
    gf_ca_invoke_command(1, 1000, тело 544)     ← ПОКА ТАКТ ВКЛЮЧЁН
    gf_hal_control_spi_clock(0)   → ioctl DISABLE_SPI_CLK (0x6706)
    при отказе — reset + usleep(5000)
```

⚠️ **Такт SPI держится только на время команды** — включён на вызов,
выключен сразу после. Это не «один раз при открытии», а на каждый кадр.

## 26.3 Порядок при закрытии ✅BIN (`gf_hal_common_close` @`0x11bcb0`)

```
destroy_timer(enroll); destroy_timer(long_pressed);
gf_hal_device_disable()   → ioctl DISABLE_IRQ (0x6704),
                            ioctl GF_IOC_EXIT (0x6701),
                            ioctl UI_DEV_DESTROY (0x5502)
invoke_command(1002 EXIT, 96)
gf_ca_close_session()
gf_hal_disable_power()    → ioctl GF_IOC_DISABLE_POWER (0x6708)
gf_hal_device_close()     → close(/dev/uinput), close(/dev/goodix_fp), sem_destroy
```

## 26.4 Модуль датчика (`1022`) ✅BIN, `gf_hal_common_enable_fingerprint_module`

```
флаг = 0 (выключить):
    если уже включён → invoke(1022, тело с +0x60=0), затем DISABLE_POWER
флаг = 1 (включить):
    если уже выключен → ENABLE_POWER, затем invoke(1022, тело с +0x60=1)
```

⚠️ Питание подаётся **до** команды при включении и снимается **после** при
выключении. Порядок, обратный, дал бы команду датчику при снятом питании.

## 26.5 `GF_IOC_RESET` ✅BIN

`gf_hal_reset_chip` → `ioctl(fd, GF_IOC_RESET)`. Сброс **не** держит
GPIO напрямую: импульс формирует сам драйвер. Наша `spinel_fp_proba`
делает то же (`spinel_fp_hw_reset`: `gpiod_set_value_cansleep` с
паузами 40/10/3 мс), но по своим таймингам — стоковые тайминги **в HAL не
заданы**, их задаёт драйвер Android.

⚠️ Это важное ограничение: последовательность сброса стока **нельзя**
вычитать из `libgf_hal.so` — она в ядре Android. Наш C привёл её сам.

## 26.6 `GF_IOC_ENABLE_SPI_CLK` — по кадрам

`gf_hal_control_spi_clock(1/0)` встречается в:
* `gf_hal_open` вокруг `DETECT_SENSOR` (4 попытки);
* `gf_hal_reinit` вокруг всего круга;
* `gf_hal_irq`-путь (обработчик события) — в `hal_milan_an_series_irq`.

⚠️ Значит **без такта SPI разряды датчика не читаются** — то, что на
планшете уже подтверждено для блоков камеры, здесь действует так же.

## 26.7 Что осталось

* Тайминги сброса у стока — в **драйвере ядра Android**. ✅ Прочитаны:
  [27](27-drajver-yadra-stoka.md) — `gf_power_on` @`0xa709c8`,
  `gf_hw_reset` @`0xa70c48`, `gf_power_off` @`0xa70bf8`.
* `gf_hal_get_fw_info` при `b != 0` **не** делает `RESET` (условие
  `if (b == 0)`), то есть при уже прошитом кристалле сброс пропускается.
  ⚠️ У нас `--native-*` делает сброс всегда — это стоит сверить замером.
