# Addresses: functions, table layout, how to read them

## HAL functions needed most often ✅BIN

| function | address |
|---|---|
| `gf_hal_open` | `0x1163f8` |
| `gf_hal_close` | `0x1169a8` |
| `gf_hal_reinit` | `0x119280` |
| `gf_hal_irq` | `0x18174` |
| `gf_hal_authenticate` | `0x117240` |
| `gf_hal_set_active_group` | `0x11781c` |
| `gf_hal_common_authenticate` | `0x11d420` |
| `gf_hal_common_enroll` | `0x11d05c` |
| `gf_hal_common_enroll_success` | `0x1214e8` |
| `gf_hal_common_authenticate_success` | `0x121870` |
| `gf_hal_save` | `0x11b514` |
| `gf_hal_update_stitch` | `0x11b668` |
| `gf_hal_init_finished` | `0x11b7bc` |
| `gf_hal_common_close` | `0x11bcb0` |
| `hal_milan_select_model` | `0x635c8` (body `0x635c8…0x639fc`) |
| `hal_milan_init_model` | `0x163324` |
| `hal_milan_an_series_irq` | `0x15c9f4` |
| `irq_pre_process` | `0x15cb64` |
| `irq_process` | `0x15d52c` |

## Who hands a command to the app

| slot | name in `gf_hal` | where it goes |
|---|---|---|
| `+0xd8` | `gf_hal_invoke_command` | the device table (`g_fingerprint_device`) → `+0x18` (Milan AN) → `gf_ca_invoke_command` |

✅BIN `gf_hal_common_authenticate` @`0x11d420` and all its siblings build the
body and call `gf_hal_invoke_command(cmd, body, length)`; that in turn is the
adapter at `0x1197d0`: `(**(code**)(g_hal_function + 0xd8))(1, cmd, body,
length)` — note the leading `1` (direction/mode).

## `g_hal_function` slots ✅BIN

| slot | name |
|---|---|
| `+0x10` | `gf_hal_cancel` |
| `+0x18` | `FUN_0012cddc` (**unnamed** in the image symbol list) |
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

⚠️ `gf_hal_set_active_group` @`0x11781c` first tries slot `+0x58` and only when
it is empty calls `gf_hal_common_set_active_group` @`0x11de14`. The Milan AN
fill routine `@0x15d6bc` leaves `+0x58` empty, so `1014` is what actually goes
out.

## How to read addresses

⚠️ This image uses **two frames of reference**, and they are easy to confuse:

* `nm -D` and Ghidra give **virtual** addresses; `objdump -d <file>
  --start-address=<that address>` works correctly;
* `.data` sits `0x10000` **earlier** in the file than its virtual address —
  that is why name tables are read with a `+0x10000` correction;
* jump tables: `target = table_base + offset` (`ldrsw` + `add`), **without**
  multiplying by 4.

⚠️ The Ghidra decompiler and the code map disagree in places: at `0x1635c8`,
`objdump` already shows a different function. When in doubt, disassemble the
whole file.

## Where the stock kernel driver lives

| symbol | address |
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

⚠️ The stock kernel carries no debug information, but its name table provides
the names — and that is enough.
