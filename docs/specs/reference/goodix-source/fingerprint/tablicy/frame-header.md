# Command frame header: fields and offsets

Compiled from two sources: proba ✅SRC (its fields are named) and `goodix.rs`
✅SRC (ported from the binary HAL). This is the piece needed most often — every
frame is built from it.

## Command header (128 bytes, `SEND_MODFD_CMD`)

| offset | field | meaning |
|---|---|---|
| `+0` | — | for Goodix with a body: **physical address of the body, 32 bits** (the upper half holds flags — do not touch) ✅SRC |
| `+4` | — | flags; leave alone when the address is 32-bit ✅SRC |
| `+32` | `command` (`cmd_id`) | `1000`, `1001`, `1016`, `1086`, `1111`, … ✅SRC |
| `+36` | `DLINA` | declared body length; the shim raises it to `0x4FFA8` ✅SRC |
| `+?` | `group` | application group ✅SRC |
| `+?` | `sequence` | command number within the session (increases) ✅SRC |
| `+?` | `time` | `ktime_get_real_seconds()*1000`, 64 bits ✅SRC |
| `+?` | `dump`, `log`, `cipher` | debug fields ✅SRC |
| `+?` | `body_length` | actual body length ✅SRC |

⚠️ The exact numeric offsets (other than `+0`, `+4`, `+32`, `+36`) are kept by
proba under the names `GF_POLE_*` — read them there, in
`spinel_fp_proba.c` (`GF_POLE_TELO`, `GF_POLE_SHIFR`, `GF_POLE_ZHURNAL`,
`GF_POLE_SCHET`, `GF_POLE_GRUPPA`, `GF_POLE_VREMYA`, `GF_POLE_SVALKA`,
`GF_POLE_KOMANDA`, `GF_POLE_DLINA`).

## Reply (64 bytes)

| offset | field | meaning |
|---|---|---|
| `+0` | result | command code; `1006` means no match ✅BIN |
| `+4` | channel status | transport error, separate from the result ✅BIN |

## Event body `1016` (`0x4FFA8`)

| offset | field |
|---|---|
| `+0x4FD65` | `0x02000000` at start |
| `+0x4FD94` | `0x200` |
| `+96` | event mask |
| `+100` | operation |
| `+0x4FF9C` | step (`1` start, `2` initial, `4`, `5`) |

## Rule

Keep numbers **named**, and in one place per layer: Rust — `goodix.rs`; kernel —
`GF_POLE_*`. A bare number has no place in a contract (`AGENTS.md`: "A number in
a contract must have a name").
