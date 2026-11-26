# HAL commands: numbers, bodies, senders

Source: `libgf_hal.so` ✅BIN. Addresses as in `nm -D`.

| code | name | body | sender |
|---|---|---|---|
| `1000` | `DETECT_SENSOR` | 544 | `gf_hal_open`, `gf_hal_reinit` |
| `1001` | `INIT` | 344 | `hal_milan_an_series_init` |
| `1002` | `EXIT` | 96 | `gf_hal_common_close` |
| `1003` | `DOWNLOAD_FW` | 96 | `gf_hal_download_fw` |
| `1004` | `DOWNLOAD_CFG` | 96 | `gf_hal_reinit` (when `config[+4]==0`), `fw_cfg` retries |
| `1005` | `INIT_FINISHED` | 100 | `gf_hal_init_finished` |
| `1006` | `PRE_ENROLL` | 104 | `gf_hal_common_pre_enroll` |
| `1007` | `ENROLL` | 176 | `gf_hal_common_enroll` |
| `1008` | `POST_ENROLL` | 96 | `gf_hal_common_post_enroll` |
| `1009` | `CANCEL` | 100 | `hal_milan_an_series_cancel` |
| `1010` | `AUTHENTICATE` | 120 | `gf_hal_common_authenticate` |
| `1012` | `SAVE` | 108 | `gf_hal_save` |
| `1013` | `REMOVE` | 192 | `gf_hal_common_remove_without_callback` |
| `1014` | `SET_ACTIVE_GROUP` | 100 | `gf_hal_common_set_active_group` |
| `1015` | `ENUMERATE` | 180 | `gf_hal_common_enumerate` |
| `1016` | `IRQ` | `0x4ffa8` | `irq_pre_process` |
| `1019` | `ESD_CHECK` | 100 | `FUN_00119fc8` |
| `1021` | `CAMERA_CAPTURE` | 96 | `gf_hal_common_camera_capture` |
| `1022` | `ENABLE_FINGERPRINT_MODULE` | 100 | `gf_hal_common_enable_fingerprint_module` |
| `1023` | `ENABLE_FF_FEATURE` | 100 | `gf_hal_common_enable_ff_feature` |
| `1071` | `NAVIGATE` | 100 | `gf_hal_common_navigate` |
| `1072` | `DETECT_NAV_EVENT` | 104 | `FUN_0015c084` |
| `1073` | `NAVIGATE_COMPLETE` | 96 | `gf_hal_nav_complete` |
| `1078` | `AUTHENTICATE_FIDO` | 172 | `gf_hal_common_authenticate_fido` |
| `1085` | `UPDATE_STITCH` | 104 | `gf_hal_update_stitch` |
| `1086` | `AUTHENTICATE_FINISH` | 120 | `FUN_001607f0` |
| `1089` | `GET_DEV_INFO` | 5496 | `gf_hal_milan_dump_chip_init_data` (debug) |
| `1090` | `LOCKOUT` | 100 | `gf_hal_common_lockout` / `reset_lockout` |
| `1091` | `PAUSE_ENROLL` | 100 | `gf_hal_common_pause_enroll` / `resume_enroll` |
| `1110` | `MT_BASE` | 29 672 | `gf_hal_mt_base_load_mt_base_to_ta` |
| `1111` | `INIT_MODEL` | `0xc3568` | `hal_milan_init_model` |

## Bodies worth remembering

* **`1000`**: `+0x218` holds the attempt counter; the reply from `+0x60` is
  `g_hal_config`.
* **`1006`**: returns the **challenge** — 8 bytes at `+0x60` of the reply body.
* **`1007`**: HAT `0x45` (69) bytes at `+0x6a`; `+0x69` = 0.
* **`1010`**: `+0x60` group, `+0x68` challenge (8 B), `+0x70` screen state
  (`-1`; the HAL does not change it).
* **`1012`**: `+0x60` group, `+100` `finger_id`.
* **`1013`**: `+0x60` group, `+100` `finger_id`; the reply carries up to 10
  pairs (slot `+0x6c`, `finger_id` `+0x94`).
* **`1016`**: step at `+0x4ff9c`; `+96` mask, `+100` operation.
* **`1086`**: `+0x6c` group, `+0x70` `finger_id`, `+100` operation, `+0x68`
  mask; the reply carries `save_flag@+0x74`, `update_stitch@+0x75`.
* **`1111`**: weights from `+0x60`, word count `+0xc3560`, kind `+0xc3564`.
