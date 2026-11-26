# 01 — HAL lifecycle

Everything below is what the **stock** stack does (`libgf_hal.so`) and what our
code does of it. Addresses are ✅BIN:
`objdump -d libgf_hal.so --start-address=<address from nm>`.

## 1. Open (`gf_hal_open` @`0x1163f8`)

```
gf_hal_device_open          open("/dev/goodix_fp"), semaphore, uinput
gf_ca_open_session          /dev/qseecom, LOAD_APP goodixfp
gf_hal_enable_power         ioctl 0x6707
gf_hal_get_fw_info          ioctl 0x8001670b  → 1 byte
if byte == 0 → RESET        ioctl 0x6702
DETECT_SENSOR (1000)        544 bytes, +0x218 holds the attempt counter 4…1
    while the command runs: ENABLE_SPI_CLK (0x40046705), then DISABLE (0x6706)
    on failure: RESET + usleep(5000), up to 4 attempts
memcpy(g_hal_config, reply+0x60, 440)   ← this chooses the chip generation
gf_hal_function_init        select the generation table (Milan AN and others)
generation init (@0x15da10): INIT (1001, 344) → … → INIT_FINISHED (1005, 100)
                             → conditional 1016 (step 2) → model (1111)
```

⚠️ On open, `RESET` happens **only if `GET_FW_INFO` returned 0**.

## 2. Close (`gf_hal_common_close` @`0x11bcb0`)

```
destroy_timer(enroll); destroy_timer(long_press)
gf_hal_device_disable    DISABLE_IRQ (0x6704) → GF_IOC_EXIT (0x6701) → UI_DEV_DESTROY
EXIT (1002, 96 bytes)
gf_ca_close_session
gf_hal_disable_power     DISABLE_POWER (0x6708)
gf_hal_device_close      close(/dev/uinput), close(/dev/goodix_fp)
```

## 3. Restart (`gf_hal_reinit` @`0x119280`)

Close the session → open it → `DETECT_SENSOR` up to **5 attempts** (the counter
stays in a register, **not in the body**) → `g_hal_config` → `INIT` → when
`config[+4] == 0` also `1004` → `1005` → `1111` → when `config[0x1b0]` is set,
`1110`.

## 4. Enrolment

| step | command | body | contents |
|---|---|---|---|
| 1 | `1014` group | 100 | `+0x60` group |
| 2 | `1006 PRE_ENROLL` | 104 | zeros; **the reply returns the challenge** (8 B at `+0x60`) |
| 3 | `1007 ENROLL` | 176 | `+0x60` group, `+0x69` 0, **HAT 69 B at `+0x6a`** |
| 4 | `1016` up to three times | `0x4ffa8` | the event |
| 5 | `1012 SAVE` when `samples_remaining == 0` | 108 | group `+0x60`, `finger_id` `+100` |
| 6 | `1008 POST_ENROLL` | 96 | zeros |

⚠️ **Without `1012` the template never reaches the disk.** That is what
produced error `1035` earlier.

## 5. Matching

```
1014 → 1010 (120: group, challenge, -1) → 1016 event → parse
     → 1086 (120: group, finger_id, operation, mask)
     → if save_flag (+0x74) → 1012
     → if update_stitch (+0x75) → 1085 (104)
     → 1009 cancel on failure or timeout
```

## 6. Cancel and pause

| action | command | body | condition |
|---|---|---|---|
| cancel | `1009` | 100 | — |
| remove | `1013` | 192 | — |
| pause enrolment | `1091` | 100, `+0x60=1` | `config[0x1af] == 0` or `power_down != 1` |
| resume enrolment | `1091` | 100, `+0x60=0` | same |
| lockout | `1090` | 100, `+0x60=1` lock, `0` unlock | — |
