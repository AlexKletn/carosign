# 08 — HAL стока: шина к приложению `libgf_ca.so`

Библиотека между `libgf_hal.so` и `libQSEEComAPI.so` ✅BIN (строки и адреса
из `libgf_hal`-набора `linux-rom/fingerprint/hal-harness/staging/libgf_ca.so`).

## Внешние имена (✅BIN `nm -D`)

| адрес | имя |
|---|---|
| `0x238c` | `gf_ca_open_session` |
| `0x25fc` | `gf_ca_close_session` |
| `0x2710` | `gf_ca_invoke_command` |
| `0x3210` | `get_key_from_keymaster` |
| `0x406c` | `gf_ca_tee_auth` |
| `0x4224` | `gf_ca_get_logbuf_length` |
| `0x4230` | `gf_ca_logdump` |

Ещё есть свой AES (`aes_cbc_encrypt/_decrypt`) и `do_xor_hi_lo` — шифрование
груза к приложению (не читано, ✅неизв.).

## Строки, по которым восстанавливается порядок ✅BIN

```
gx_ta_send_modified_cmd_req success / fail, ret=%d
gf_ta_send_modified_cmd_req
QSEECom_send_modified_cmd failed with retval = %d
begin token %d, cmd_id=%d
send_hmac_key
keymaster64 / [gf_keymaster] / get_key_from_keymaster
keymaster start success/fail, km handle=%p, errno=%d
```

Отсюда картина:

1. `gf_ca_open_session` открывает `/dev/qseecom`, делает `LOAD_APP goodixfp`
   (через `libQSEEComAPI`) и заводит SB-память.
2. `gf_ca_tee_auth` — аутентификация сеанса; `send_hmac_key` и
   `get_key_from_keymaster` — ключ приходит **из keymaster64** (не из датчика).
   ⚠️ Это объясняет, почему без `km41` (keymaster) часть путей не идёт.
3. `gf_ca_invoke_command(cmd_id, тело)` — главный вызов. Кадр: заголовок
   128 байт, в нём `cmd_id` и длина; тело — отдельно, в **ion-буфере**
   (`gf_sec_ion_malloc` @`0x20e0` plt). Передаётся
   `QSEECom_send_modified_cmd` (`0x10e8` plt): так тело попадает в физический
   адрес, а не копией.
4. Ответ: 64 байта заголовка (результат `+0`, ошибка канала `+4`) и тело
   (у `1016` — `0x4FFA8`).

## Что важно для Rust

* `cmd_id` хранится в заголовке Body Command (`+32`), длина — `+36`
  (у шима `DLINA`), адрес тела — в начале заголовка (32 бита для Goodix).
* Ключ сеанса берётся у keymaster64, а не у датчика. Если keymaster не
  загружен — `get_key_from_keymaster` падает и падает вся авторизация.
* `begin token … cmd_id=%d` — признак, что для каждого вида команды печатается
  свой след; он полезен как опорная точка при сверке журналов.
