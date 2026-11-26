# 27 — Драйвер ядра стока: питание, сброс, IRQ

Источник — **стоковое ядро в образе**: `source_artifacts/dumps/tb371fc-stock/
boot_unpacked/kernel`. Таблица имён разобрана `tools/spinel-kallsyms.py`
(121 415 имён). Тела функций читаются `objdump -D -b binary` по адресу
из таблицы. ✅BIN

Это закрывает то, чего нет в HAL: **точный порядок питания и сброса**.

## 27.1 Где живёт драйвер

| символ | адрес |
|---|---|
| `gf_ioctl` | `0xa6f8d0` |
| `gf_open` / `gf_release` | `0xa70020` / `0xa701c0` |
| `gf_probe` | `0xa702e8` |
| `gf_irq` | `0xa70280` |
| `sendnlmsg` | `0xa70d28` |
| `gf_parse_dts` | `0xa70818` |
| `gf_power_on` | `0xa709c8` |
| `gf_power_off` | `0xa70bf8` |
| `gf_hw_reset` | `0xa70c48` |
| `gf_irq_num` | `0xa70cf0` |
| `gf_fops` | `0x17951d0` |

⚠️ В стоковом ядре нет отладочных сведений, но таблица имён даёт имена —
этого достаточно.

## 27.2 `gf_power_on` @`0xa709c8` ✅BIN

GPIO сброса в этой функции **не трогается вовсе**.

```asm
__const_udelay(0x008958) × 41;                        /* 1 441 560 единиц */
regulator_set_voltage(vdd, 0x2dc6c0, 0x2dc6c0);       /* 3 000 000 мкВ */
regulator_enable(vdd);                                /* при отказе — printk и выход */
__const_udelay(0x008958) × 15;                        /* 527 400 */
gpiod_set_raw_value(reset, 1);                        /* RST ПОДНЯТ */
__const_udelay(0x008958) × 25;                        /* 879 000 */
gpiod_set_raw_value(reset, 0);                        /* RST ОПУЩЕН */
__const_udelay(0x008958) × 6;                         /* 210 960 */
```

⚠️ `gpiod_set_raw_value` — **сырое** значение: `1` — это «напряжение
высокое» на выводе, без учёта `GPIO_ACTIVE_LOW`. Так что знаки в нашем
C (через `gpiod_set_value_cansleep`) относятся к тем же номерам наоборот.

## 27.3 `gf_hw_reset(u32 ms)` @`0xa70c48` ✅BIN

```asm
gpiod_set_raw_value(reset, 1);       /* высокий */
gpiod_set_raw_value(reset, 0);       /* низкий */
__const_udelay(0x00c6c0);            /* 50 880 единиц удержания */
gpiod_set_raw_value(reset, 1);       /* снова высокий */
if (ms) __const_udelay(0x008958) × ms;   /* ожидание после */
```

⚠️ **Порядок импульса: `1 → 0 → пауза → 1`**, то есть активный низкий
уровень `0` сначала **поднимается**, а потом опускается. Это осознанный
фронт для кристалла, а не случайность.

## 27.4 `gf_power_off` @`0xa70bf8` ✅BIN

```asm
regulator_disable(vdd);
__const_udelay(0x008958) × 1;
```

## 27.5 `gf_irq` @`0xa70280` и `sendnlmsg` @`0xa70d28` ✅BIN

`gf_irq`: `pm_wakeup_ws_event` → `sendnlmsg`. Внутри `sendnlmsg`:
`__alloc_skb` → `__nlmsg_put` → отправка → `kfree_skb`. То есть **событие
пальца уходит как netlink-сообщение** — тот же способ, что у нас на
unit 29.

## 27.6 `gf_ioctl` @`0xa6f8d0` ✅BIN

```
if ((cmd & 0xff00) != 0x6700) return -ENOTTY;
проверка доступа к пользовательской области;
switch по таблице переходов @0x17951a8   /* 11 записей: 0x6700…0x670b */
```

⚠️ Набор команд драйвера и HAL — один и тот же (`0x6700…0x670b`), значит
наши ioctl'ы в заглушке не «выдуманные», а ровно эти.

## 27.7 Наш C и сток: что расходится ✅SRC против ✅BIN

`spinel_fp_razbudit` (`spinel_fp_proba.c:610`):

| шаг | сток | наш C |
|---|---|---|
| перед питанием | 41 единица | `msleep(40)` |
| питание | `set_voltage(3 000 000)` + `enable` | `enable` |
| после рельсы | 15 единиц | нет |
| импульс RST | `1 → 25 единиц → 0` | `1 → 3 мс → 0` |
| после импульса | 6 единиц | `msleep(10)` |

`spinel_fp_hw_reset` (`:665`): `0 → 10 мс → 1 → 3 мс → 0`.
Сток: `1 → 0 → пауза → 1`. **Фронт другой.**

⚠️ Вывод: порядок в нашем C — **не стоковый**. Если сброс у нас
срабатывал, то по другой причине (например, держался дольше). Это
первый кандидат на объяснение «после перезагрузки иногда не поднимается».

## 27.8 Дерево: откуда драйвер берёт выводы и рельсу ✅BIN

`source_artifacts/dumps/tb371fc-stock/dts/dtbo-02.dts`, `fragment@73`
(узел `goodix_fp`):

| свойство | значение |
|---|---|
| `compatible` | `goodix,fingerprint` |
| `interrupt-parent` | `&tlmm` |
| `interrupts` | `<0x17 0x00>` — **вывод 23**, тип 0 |
| `fp-gpio-irq` | `<&tlmm>, <0x17 0x00>` — тот же **23** |
| `fp-gpio-reset` | `<&tlmm>, <0x55 0x00>` — **вывод 85** |
| `goodix_vdd-supply` | `&pm8150a_l11` |

Рельса `pm8150a_l11` (`boot-00.dts:4498`): `regulator-max-microvolt =
0x326a40` (3 300 000 мкВ), в драйвере ставится ровно **3 000 000**.

`gf_parse_dts` @`0xa70818` читает: `of_get_named_gpio_flags` для
`goodix_reset`/`goodix_irq` строк, `regulator_get(vdd)`,
`devm_gpio_request`, `gpiod_direction_output_raw` (сброс) и
`gpiod_direction_input` (IRQ).

⚠️ **Имена файлов свойств не совпадают с именами в коде**: в дереве
`fp-gpio-reset`, в строке драйвера `goodix_reset`. Значит либо драйвер
читает по номерам DTS-свойств, либо строка — от другой ревизии.
Для нас это неважно: **выводы 23 (IRQ) и 85 (RST), рельса `pm8150a_l11`**.

## 27.9 Чего ещё нет

* Единица `0x8958` в мс не переводится без `HZ` стокового ядра. Отношения
  (`41 : 15 : 25 : 6` и `50 880`) — доказаны, абсолютные миллисекунды — нет.
