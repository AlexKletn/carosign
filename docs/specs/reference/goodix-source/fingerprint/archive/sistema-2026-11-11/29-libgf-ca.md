# 29 — `libgf_ca`: шифрование канала отпечатка (по строкам и коду)

`libgf_ca.so` — тонкий канал между HAL и доверенным приложением. Здесь то,
что раньше было «неизвестно»: whitebox, AES, подпись запроса/ответа.

## 29.1 Устройство библиотеки ✅BIN (`readelf`, `nm -D`)

| адрес | имя |
|---|---|
| `0x238c` | `gf_ca_open_session` |
| `0x25fc` | `gf_ca_close_session` |
| `0x2710` | `gf_ca_invoke_command` |
| `0x3210` | `get_key_from_keymaster` |
| `0x406c` | `gf_ca_tee_auth` |
| `0x4224` | `gf_ca_get_logbuf_length` |
| `0x4230` | `gf_ca_logdump` |
| — | `aes_cbc_encrypt` `0x9dd4`, `aes_cbc_decrypt` `0xa5a4` |
| — | `copyBlock` `0x7078`, `copyBytes` `0x70a4`, `do_xor_hi_lo` `0x4c28` |

`.rodata` начинается с `0xaca0`, `.text` — с `0x2340`.

## 29.2 Whitebox ✅BIN (строки)

```
[%s] whitebox is enabled.        @0xaf69
[%s] whitebox is disabled.       @0xaf83
```

⚠️ В образе **ветка whitebox есть**, и от неё зависит, шифруется ли тело.
Обе ветви ведут в `gf_ta_send_modified_cmd_req`:

```
[%s] gx_ta_send_modified_cmd_req success      @0xae2a
[%s] gx_ta_send_modified_cmd_req fail, ret=%d @0xae90
[%s] QSEECom_send_modified_cmd failed with retval = %d  @0xaf9e
```

## 29.3 Что шифруется — по строкам и именам ✅BIN

| строка | смысл |
|---|---|
| `[%s] begin token %d, cmd_id=%d` | перед отправкой печатается **токен** и номер команды |
| `[%s] Check command return data digest failed.` | у ответа проверяется **дайджест (подпись)** |
| `[%s] The size of command data for ca_auth_confirm command must be aligned with AES block.` | размер тела для `ca_auth_confirm` должен быть **кратен блоку AES** (16) |
| `send_hmac_key` / `keymaster64` / `[gf_keymaster]` | ключ приходит от `keymaster64` |

Отсюда прямые выводы, которые раньше были догадками:

1. **Тело команды шифруется AES-CBC** (есть `aes_cbc_encrypt/_decrypt`,
   требование кратности блоку, и `do_xor_hi_lo` для IV/цоколя).
2. **Ответ проверяется дайджестом** — не только статусом; `Check command
   return data digest failed` — отдельный отказ.
3. **Ключ — не от датчика**: `get_key_from_keymaster` берёт его у
   `keymaster64`, и без загруженного keymaster канал не настроится.
4. **Перед каждой командой идёт токен** (`begin token %d`).

## 29.4 Почему у нас это ещё не мешало

Наш путь (proba + Rust) **не пользуется `libgf_ca`**: мы шлём кадр
напрямую через `/dev/qseecom` и **без шифрования**. Стоковый HAL тоже
работает без него, когда whitebox **выключен** — и именно на этот режим
ссылался `goodix.rs` («белый ящик выключен»).

⚠️ Значит шифрование — **необязательная ветвь** для обычного устройства, но
**обязательная** для тех TA, что требуют подписанных запросов. Наш датчик
таких не требует (замеры ✅MSR проходили без AES). Спецификация это
фиксирует, чтобы не искать «сломанный шифр» там, где его нет.

## 29.5 Порядок открытия сеанса ✅BIN (по строкам)

```
gf_ca_open_session: open /dev/qseecom → QSEECom_start_app("goodixfp")
gf_ca_tee_auth:     send_hmac_key + get_key_from_keymaster → ключ
gf_ca_invoke_command(cmd_id, тело):
    при whitebox: AES-CBC(+ XOR) тела → «digest» в ответе
    gf_ta_send_modified_cmd_req → QSEECom_send_modified_cmd
    проверка: retval; при успехе — дайджест ответа
```

## 29.6 Чего не хватает

* Ровно **какой ключ** и **какой режим AES** (CBC с нулевым IV или с
  вектором из `do_xor_hi_lo`) — в строках нет; нужен разбор
  `aes_cbc_encrypt` + вызовов. Пока это не нужно нашему пути.
* Формат «дайджеста» ответа — `Check ... digest failed` есть, размер и
  алгоритм — нет.
