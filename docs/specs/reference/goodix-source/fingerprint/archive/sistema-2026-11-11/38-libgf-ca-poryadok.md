# 38 — `libgf_ca`: порядок сеанса и что именно шлётся

Продолжение [29](29-libgf-ca.md). Здесь — по строкам и по вызовам, без
выдумок.

## 38.1 Функции и их резолвы

`libgf_ca.so`, `nm -D` и строки ✅BIN:

| что | адрес | подтверждение |
|---|---|---|
| `gf_ca_open_session` | `0x238c` | `QSEECom_start_app` (`f15` в строках) |
| `gf_ca_close_session` | `0x25fc` | `ada7` |
| `gf_ca_invoke_command` | `0x2710` | `acf9`/`acf5` |
| `get_key_from_keymaster` | `0x3210` | `f9f`, строки `[gf_keymaster]`, `keymaster64` |
| `gf_ca_tee_auth` | `0x406c` | `1036` |
| `gf_ca_logdump` | `0x4230` | — |

Резолвы (`QSEECom_*`, `__system_property_get`, `gf_sec_ion_malloc`) идут
через PLT (`0x1ed0…0x2340`), `.text` — `0x2340…0xac9c`, `.rodata` —
`0xaca0…`.

## 38.2 Что читается по строкам (и чему верить)

| строка | смысл |
|---|---|
| `[%s] gx_ta_send_modified_cmd_req success` | удачная отправка тела |
| `[%s] gx_ta_send_modified_cmd_req fail, ret=%d` | отказ отправки, код |
| `[%s] QSEECom_send_modified_cmd failed with retval = %d` | отказ QSEECom |
| `[%s] ta execution succeed` / `ta execution failed, errno=%d` | итог у TA |
| `[%s] begin token %d, cmd_id=%d` | у каждой команды есть токен |
| `[%s] Check command return data digest failed.` | проверка дайджеста ответа |
| `[%s] The size of command data for ca_auth_confirm command must be aligned with AES block.` | тело `ca_auth_confirm` кратно 16 |
| `[%s] whitebox is enabled.` / `disabled.` | режим шифрования |
| `[%s] keymaster start success,km handle=%p, errno=%d` | сеанс keymaster поднят |
| `[%s] get key success.` / `get key failed.` | ключ от keymaster |
| `[%s] gf_sec_ion_malloc failed with retval = %d` | память под тело |

## 38.3 Порядок сеанса (по строкам, не по догадке)

1. `gf_ca_open_session` → `QSEECom_start_app` и `gf_sec_ion_malloc`.
2. `gf_ca_tee_auth` → `get_key_from_keymaster` (`keymaster start` → `get key`).
3. Каждая команда: `begin token`, затем `gf_ta_send_modified_cmd_req`
   (`QSEECom_send_modified_cmd`), затем проверка дайджеста и итога TA.
4. При `whitebox enabled` — шифрование тела и/или токена AES.

## 38.4 Чего не установлено

* Какой ключ выдаёт keymaster (`gf_keymaster` — подсказка: не «сырой» ключ
  датчика).
* Где именно применяется AES: к телу, к токену, к обоим.
* Формат «дайджеста» ответа (длина/алгоритм).

⚠️ Для **нашего** пути это не нужно: наш датчик работает при выключенном
whitebox, и замеры ✅MSR это подтверждают. Записано, чтобы этот вопрос не
считался «неизвестной угрозой».
