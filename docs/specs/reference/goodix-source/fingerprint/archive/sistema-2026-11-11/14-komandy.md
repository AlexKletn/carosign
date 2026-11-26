# 14 — Команды HAL: полный перечень отправляемых

Сводка по декомпиляции `libgf_hal.so` ✅BIN: все места `gf_hal_invoke_command(...)`
в `/home/alex/ghidra-work/decomp-*.c`, с функцией-отправителем. Имена — из
`specs/stock/fingerprint/PERECHNI.txt`.

| код | имя по перечню | тело | кто шлёт (адрес) |
|---|---|---|---|
| `1000` (`0x3e8`) | `DETECT_SENSOR` | 544 | `hal_milan_an_series_init` @`0x15da10` |
| `1001` (`0x3e9`) | `INIT` | 344 | `hal_milan_an_series_init` @`0x15da10` |
| `1005` (`0x3ed`) | `INIT_FINISHED` | 100 | `gf_hal_init_finished` @`0x11b7bc` |
| `1006` (`0x3ee`) | `PRE_ENROLL` | 104 | `gf_hal_common_pre_enroll` @`0x11be8c` |
| `1007` (`0x3ef`) | `ENROLL` | 176 | `gf_hal_common_enroll` @`0x11d05c` |
| `1010` (`0x3f2`) | `AUTHENTICATE` | 120 | `gf_hal_common_authenticate` @`0x11d420` |
| `1012` (`0x3f4`) | **`SAVE`** | **108** | `gf_hal_save` @`0x11b514` |
| `1013` (`0x3f5`) | `REMOVE` | 192 | `gf_hal_common_remove_without_callback` @`0x11d77c` |
| `1014` (`0x3f6`) | `SET_ACTIVE_GROUP` | 100 | `gf_hal_common_set_active_group` @`0x11de14` |
| `1015` (`0x3f7`) | `ENUMERATE` | 180 | `gf_hal_common_enumerate` @`0x11e000` |
| `1016` (`0x3f8`) | `IRQ` | `0x4ffa8` | событие (см. [13](13-sobytie-razbor.md)) |
| `1019` (`0x3fb`) | `ESD_CHECK` | 100 | `FUN_00119fc8` @`0x119fc8` |
| `1022` (`0x3fe`) | `ENABLE_FINGERPRINT_MODULE` | 100 | `gf_hal_common_enable_fingerprint_module` @`0x11eb78` |
| `1023` (`0x3ff`) | `ENABLE_FF_FEATURE` | 100 | `gf_hal_common_enable_ff_feature` @`0x11ee14` |
| `1071` (`0x42f`) | `NAVIGATE` | ? | `gf_hal_common_navigate` @`0x11e9b4` |
| `1072` (`0x430`) | `DETECT_NAV_EVENT` | 104 | `FUN_0015c084` |
| `1073` (`0x431`) | `NAVIGATE_COMPLETE` | ? | `gf_hal_nav_complete` @`0x11a4b4` |
| `1085` (`0x43d`) | **`UPDATE_STITCH`** | **104** | `gf_hal_update_stitch` @`0x11b668` |
| `1086` (`0x43e`) | `AUTHENTICATE_FINISH` | 120 | `FUN_001607f0` @`0x1607f0` |
| `1089` (`0x441`) | `GET_DEV_INFO` | 5496 | `gf_hal_milan_dump_chip_init_data` |
| `1090` (`0x442`) | `LOCKOUT` | ? | `gf_hal_common_reset_lockout` @`0x11efa0` |
| `1091` (`0x443`) | `PAUSE_ENROLL` | ? | `gf_hal_common_pause_enroll` @`0x1203b4` |
| `1110` (`0x456`) | `MT_BASE` | 29672 | `gf_hal_mt_base_load_mt_base_to_ta` |
| `1111` (`0x457`) | `INIT_MODEL` | `0xc3568` | `hal_milan_init_model` @`0x163324` |

Полный порядок шагов — [16](16-posledovatelnost.md).

Ещё `1078` (`GF_CMD_AUTHENTICATE_FIDO`) — из `gf_hal_common_authenticate_fido`
@`0x11fb24`: тело **`0xac` (172)**. Для входа в систему не нужно; в перечне
держится ради полноты.

## ⚠️ Чего в Rust нет, а сток шлёт обязательно

В `goodix_common_authenticate_success` @`0x121870` (наша дорогая ветка —
успех распознавания):

```c
if (body[0x4fcd0])  rc = gf_hal_save(group, finger_id);      /* 1012 */
if (body[0x4fcd1])  gf_hal_update_stitch(group, finger_id);  /* 1085 */
```

А `FUN_00160ae0`-ветка успеха (`FUN_001603bc` @`0x1603bc`) в конце **возвращает
сырой** `gf_hal_invoke_command(0x43e)` в `*param_2`, то есть код результата
`1086`.

⚠️ Следствие: если "опознано", но `save_flag` в ответе `1086` равён 1, сток
дописывает шаблон (`1012`) и обновляет стежок (`1085`). Без этого
самообучение не идёт, и следующий сеанс. **В `goodix.rs` этих команд нет.**
Это же объясняет, почему в `libgf_hal.so` есть символ `gf_hal_save`, а в
нашем плане переноса — нет.

## Порядок на успехе (как в стоке)

```
1016 → (ступень 4 → ещё 1016) → (ступень 5 → ещё 1016)
     → ответ с маской 0x80 и оператором 1102/1103
     → FUN_0015fad4: результат 0
     → FUN_001603bc: notify_succeeded(group, finger_id, token+0x4fd4c)
                     FUN_001607f0 → 1086 → save_flag/update_stitch_flag
     → если save_flag:     1012 SAVE
     → если update_stitch: 1085 UPDATE_STITCH
```

## Что исправить в спецификации и в Rust

1. В таблицу [07](07-hal-sostoyaniya.md) добавить `1012`, `1013`, `1085`,
   `1090` — сейчас их нет.
2. В `10-rust-dolzhen.md` пункт «успех — по `1086`» дополнить: после `1086`
   читать `+0x74`/`+0x75` и, при ненулевых, слать `1012`/`1085`.
3. В `goodix.rs` поля-флаги уже есть в `FinishAuthentication` (буфер 120
   байт), но отправки `1012`/`1085` нет — это следующий перенос.
