# Names: modes, operations, navigation, errors, chips

Every table was read from `libgf_hal.so` ✅BIN: the array sits in `.data`, each
record is 16 bytes (`u32 code`, `u32 reserved`, `u64 pointer`), the pointer is a
**file offset**, so a string address is the value `+ 0x10000`.

## Operations (`operation_table` @`0x4177c8`)

| code | name |
|---|---|
| `0` | `OPERATION_ENROLL` |
| `1` | `OPERATION_AUTHENTICATE_IMAGE` |
| `2` | `OPERATION_AUTHENTICATE_FF` |
| `3` | `OPERATION_AUTHENTICATE_SLEEP` |
| `4` | `OPERATION_AUTHENTICATE_FIDO` |
| `5` | `OPERATION_FINGER_BASE` |
| `6` | `OPERATION_NAV` |
| `7` | `OPERATION_NAV_BASE` |
| `8` | `OPERATION_CHECK_FINGER_LONG_PRESS` |
| `9` | `OPERATION_HOME_KEY` |
| `10` | `OPERATION_POWER_KEY` |
| `11` | `OPERATION_CAMERA_KEY` |
| `12` | `OPERATION_HEARTBEAT_KEY` |
| `13…21` | `OPERATION_TEST_IMAGE_MODE`, `_DEBUG_MODE`, `_FF_MODE`, `_KEY_MODE`, `_NAV_MODE`, `_NAV_BASE_MODE`, `_FINGER_BASE_MODE`, `_IDLE_MODE`, `_SLEEP_MODE` |
| `22…23` | `_HBD_DEBUG_MODE`, `_HBD_MODE` |
| `24`, `24`, `25`, `26` | `_PIXEL_SHORT_STREAK`, `_PIXEL_OPEN_STEP1`, `_PIXEL_OPEN_STEP2`, `_PIXEL_OPEN_STEP3` |
| `27…28` | `_BAD_POINT_RECODE_BASE`, `_BAD_POINT` |
| `29…31` | `_PERFORMANCE`, `_SPI_PERFORMANCE`, `_SPI_TRANSFER` |
| `32…34` | `_FRR_FAR_RECORD_CALIBRATION`, `_ENROLL`, `_AUTHENTICATE` |
| `35…36` | `_UNTRUSTED_ENROLL`, `_UNTRUSTED_AUTHENTICATE` |
| `37…38` | `_CHECK_FINGER_EVENT`, `_BIO_CALIBRATION` |

⚠️ **Code `24` appears twice** (`_PIXEL_SHORT_STREAK` and
`_PIXEL_OPEN_STEP1`) — that is how the table itself is written, not a reading
error.

## Modes (`mode_table` @`0x416fa8`)

| code | name |
|---|---|
| `0` | `MODE_IMAGE` |
| `1` | `MODE_KEY` |
| `2` | `MODE_SLEEP` |
| `3` | `MODE_FF` |
| `4` | `MODE_NAV` |
| `5` | `MODE_NAV_BASE` |
| `6` | `MODE_DEBUG` |
| `7` | `MODE_FINGER_BASE` |
| `8` | `MODE_IDLE` |
| `9` | `MODE_HBD` |
| `10` | `MODE_HBD_DEBUG` |
| `11` | `MODE_IMAGE_CONTINUE` |
| `12…15` | `MODE_TEST_BAD_POINT`, `_PIXEL_OPEN_DEFAULT`, `_PIXEL_OPEN_POSITIVE`, `_BAD_POINT_FINGER_BASE` |
| `17` | `MODE_MAX` |
| `1000` | `INVALID_MODE` |

⚠️ Code `16` is absent (a hole), `17` is `MODE_MAX`, and `INVALID_MODE` sits
**past the end** of the array — that is why `gf_strmode` for an unknown code
searches up to `MODE_MAX` and takes the next node.

## Navigation (`navmode_table` @`0x416808`)

| code | name |
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

⚠️ **The order is deliberate**: `Z` comes before `XY`. That is why the plain
`nav_mode < 8` check in `gf_hal_common_navigate` does not follow the table
order, and `8` (`GF_NAV_MODE_MAX`) is not accepted.

## Errors (`err_table` @`0x416078`) — the beginning

| code | name |
|---|---|
| `0` | `GF_SUCCESS` |
| `1000` | `GF_ERROR_BASE` |
| `1001` | `GF_ERROR_OUT_OF_MEMORY` |
| `1002` | `GF_ERROR_OPEN_TA_FAILED` |
| `1003` | `GF_ERROR_BAD_PARAMS` |
| `1004` | `GF_ERROR_NO_SPACE` |
| `1005` | `GF_ERROR_REACH_FINGERS_UPLIMIT` |
| `1006` | `GF_ERROR_NOT_MATCH` |
| `1007` | `GF_ERROR_CANCELED` |
| `1008` | `GF_ERROR_TIMEOUT` |
| `1009` | `GF_ERROR_PREPROCESS_FAILED` |
| `1010` | `GF_ERROR_GENERIC` |

⚠️ **This is what the stock code returns instead of `errno`**: `1006` is
`NOT_MATCH`, `1010` is `GENERIC`. Match failure and channel failure belong to
one and the same vocabulary.

## Commands (`cmd_id_table` @`0x4170c8`)

`1000 DETECT_SENSOR`, `1001 INIT`, `1002 EXIT`, `1003 DOWNLOAD_FW`,
`1004 DOWNLOAD_CFG`, `1005 INIT_FINISHED`, `1006 PRE_ENROLL`, `1007 ENROLL`,
`1008 POST_ENROLL`, `1009 CANCEL`, `1010 AUTHENTICATE`, `1011 GET_AUTH_ID`, … —
the same sequence as in `PERECHNI.txt` and as used throughout this
documentation.

## Chips (`chip_table` @`0x416898`) ✅BIN

| code | name |
|---|---|
| `43` (`0x2b`) | `GF_CHIP_3626ZS1` |
| `44` (`0x2c`) | `GF_CHIP_3636ZS1` |
| `45` (`0x2d`) | `GF_CHIP_3988` |
| **`46` (`0x2e`)** | **`GF_CHIP_3956`** |
| `47` (`0x2f`) | `GF_CHIP_5658ZS3` |
| `48` (`0x30`) | `GF_CHIP_3976ZS1` |
| `49` (`0x31`) | `GF_CHIP_UNKNOWN` |

✅ Our sensor is **46 = `GF_CHIP_3956`**.

## Model gate (`FUN_00165c3c` @`0x165c3c`) ✅BIN

`hal_milan_init_model` only proceeds when the chip is `0x16`, one of
`0x2b…0x2e`, or `0x30`:

```c
chip = g_hal_config[0];                 /* DETECT_SENSOR body +0x60 */
ok = (chip == 0x16) || (chip - 0x2b < 4) || (chip == 0x30);
```

## Kind 1 (false-touch guard), `hal_milan_select_model` ✅BIN

| chip | symbol | words |
|---|---|---|
| `0x2c` | `DLWeightLight3966MisTouch_13_13` | 74 529 |
| `0x2e` | `DLWeightLightDubaiB4_3_0_0MisTouch` | 74 529 |
| `0x30` | `DLWeightMis_3976ZS` | 74 529 |
