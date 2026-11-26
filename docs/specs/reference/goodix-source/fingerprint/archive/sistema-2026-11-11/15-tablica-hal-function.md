# 15 — Таблица `g_hal_function` (раскладка слотов)

`gf_hal_milan_an_series_function_customize` @`0x15d6bc` заполняет массив
указателей; `gf_hal` берёт из него функции по номеру слота. Прочитано по
декомпиляции ✅BIN.

## Кто передаёт команду в приложение

| слот | имя в `gf_hal` | куда идёт |
|---|---|---|
| `+0xd8` | `gf_hal_invoke_command` | таблица устройства (`g_fingerprint_device`) → `+0x18` (Milan AN) → `gf_ca_invoke_command` |

✅BIN `gf_hal_common_authenticate` @`0x11d420` и все прочие собирают тело и
зовут `gf_hal_invoke_command(cmd, тело, длина)`; тот сам — переходник в
`0x1197d0`: `(**(code**)(g_hal_function + 0xd8))(1, cmd, тело, длина)`, то
есть добавлен первый аргумент `1` (направление/режим).

## Слоты, которые вызывают внешние части HAL

| слот | имя |
|---|---|
| `+0x10` | `gf_hal_cancel` |
| `+0x18` | `FUN_0012cddc` (**не назван** в перечне имён образа) |
| `+0x20` | `gf_hal_test_prior_cancel` |
| `+0x28` | `gf_hal_pre_enroll` |
| `+0x30` | `gf_hal_enroll` |
| `+0x38` | `gf_hal_post_enroll` |
| `+0x40` | `gf_hal_authenticate` |
| `+0x48` | `gf_hal_get_auth_id` |
| `+0x50` | `gf_hal_remove` |
| `+0x58` | `gf_hal_set_active_group` |
| `+0x60` | `gf_hal_enumerate` |
| `+0x68` | `gf_hal_enumerate_with_callback` |
| `+0x70` | `gf_hal_irq` |
| `+0x78` | `gf_hal_screen_on` |
| `+0x80` | `gf_hal_screen_off` |
| `+0x88` | `gf_hal_set_safe_class` |
| `+0x90` | `gf_hal_navigate` |
| `+0x98` | `gf_hal_enable_fingerprint_module` |
| `+0xa0` | `gf_hal_camera_capture` |
| `+0xa8` | `gf_hal_enable_ff_feature` |
| `+0xb0` | `gf_hal_enable_bio_assay_feature` |
| `+0xb8` | `gf_hal_start_hbd` |
| `+0xc0` | `gf_hal_reset_lockout` |
| `+0xd0` | `gf_hal_sync_finger_list` |
| `+0xd8` | `gf_hal_invoke_command` |
| `+0x100` | `gf_hal_authenticate_fido` |
| `+0x108` | `gf_hal_is_id_valid` |
| `+0x110` | `gf_hal_get_id_list` |
| `+0x118` | `gf_hal_pause_enroll` |
| `+0x120` | `gf_hal_resume_enroll` |

## Что из этого важно для переноса

1. `gf_hal_set_active_group` @`0x1781c` **сначала пробует слот `+0x58`**, и
   только если он пуст (не заведён) — зовёт `gf_hal_common_set_active_group`
   @`0x11de14`. Отсюда: **какая команда реально уходит, зависит от того, чем
   заполняется слот `+0x58`** для Milan AN. В перечне заполнения `@0x15d6bc`
   слота `+0x58` нет — значит `0x58` пуст и идёт `1014` (наше `goodix.rs`
   совпадает).
2. Отмена у нас `goodix.rs` шлёт как `1009` — это `GF_CMD_CANCEL`
   (`0x3f1`) ✅. Удаление шаблона при этом идёт **не** через
   `gf_hal_cancel`, а через `1013 REMOVE` из `FUN_0015c084`; держать в
   голове при переносе удаления.
3. Слот `+0xd8` принимает 4-е аргумента: первый `1`. В Rust `encode` не
   передаёт этот флаг — его учитывает шим/`gf_ca`, а не HAL.

⚠️ **Что это не говорит.** Этим заходом раскладка слотов известна, но
**сам массив** `g_hal_function` в образе всё равно не нужен: его заполняет
`function_customize` во время `gf_hal_open`, и порядок слотов виден по
вызовам. Полной адресной сверки массива (значений `+0x58` и `+0x70`)
не требуется, пока не придётся переносить `gf_hal_remove`/`gf_hal_enroll`.
