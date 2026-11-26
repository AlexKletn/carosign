# 02 — Sensor event (`1016`)

The subtlest part, and the one around which most mistakes clustered.

## 1. What arrives from the kernel

The event is a **netlink message** from the driver/proba (unit 29, payload
`[1]`). It is not a packet from the TZ: the TZ takes no part in event delivery.

## 2. What the HAL does (`hal_milan_an_series_irq` @`0x15c9f4`)

```
body = calloc(0x4ffa8)
irq_pre_process  @0x15cb64   — fill in and send 1016
irq_process      @0x15d52c   — branches by mask
```

### `irq_pre_process` — the body is zeroed EVERY time

```
body[0x4fd4c] = 0
body[0x4fd65] = 0x02000000
body[0x4fd94] = 0x200
body[0x4ff9c] = 1                    ← step
1016 (body 0x4ffa8)
if the reply left step 4 → one MORE 1016 with the same body
if step 5                → one MORE 1016
```

⚠️ There are no further repeats. On the first reply (step `2`) there is a
single call.

### `irq_process` — branches by mask `+0x60`

| operation `+100` | what it does |
|---|---|
| `0,1,2,4` | `FUN_0015f880`: mask bits |
| `5` | logs "finger base" |
| `6` | navigation |
| `9,10,11` | `gf_hal_common_irq_key` |
| others | test branch |

`FUN_0015f880`: `bit 1 (0x2)` — touch down, `bit 7 (0x80)` — image,
`bit 2 (0x4)` — touch up.

## 3. Reply fields

| offset | field |
|---|---|
| `+0` | command result |
| `+4` | channel status |
| `+96` | **mask** |
| `+100` | **operation** (0 enrol, 1 match, 2 fast match, 4 FIDO, 6 navigation…) |
| `+104` | **operator** — the command that was read (`1064`, `1102`, `1103`) |
| `+0x4fcc0` | group |
| `+0x4fcc4` | `finger_id` |
| `+0x4fcc8` | `samples_remaining` (enrolment) |
| `+0x4fcd0` / `+0x4fcd1` | `save_flag` / `update_stitch` from `1086` |
| `+0x4fd4c` | start of the token (8 bytes) |

## 4. The matching result

✅ "Matched" is set only when the mask has `0x80`, the operation is `1`/`2`,
and the **operator was read** (`1102`/`1103`/`1064`). Code `1006` is
`GF_ERROR_NOT_MATCH` (no match), and `1010` is `GF_ERROR_GENERIC`.

## 5. What our Rust does

The same: one `1016` per edge, mask and operation parsed separately, repeats
only on steps `4`/`5`, `1086` after success, then `1012`/`1085` by the flags.

⚠️ **Not understood:** why the direct path receives no second edge after the
first touch (mask `0x2`), while the HAL receives a stream. Code analysis showed
no difference — this is a measurement to be taken during implementation.
