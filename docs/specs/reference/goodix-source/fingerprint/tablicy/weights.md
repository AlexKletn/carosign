# Weights: symbols, sizes, what is actually loaded

## Which symbols exist ✅BIN (`readelf -sW`)

### Main filters (`…Filter`), 8-bit

| symbol | address | size | GOT slot |
|---|---|---|---|
| `DLWeightLight3956Filter` | `0xd10c8` | 298 116 | **not in GOT** |
| `DLWeightLightDubaiAFilter` | `0x119d4c` | 298 116 | `0xd0e10` |
| `DLWeightLightChicagoCUFilter` | `0x1629d0` | 298 116 | `0xd0f38` |
| `DLWeightLightDubaiS200_116Filter` | `0x1ab654` | 298 116 | **not in GOT** |

⚠️ `3956Filter` and `DubaiS200_116Filter` do not appear in `.rela.dyn`, so the
code never fetches them through the GOT. Had the main filter of our chip been
`3956Filter`, its slot would be there.

### Main filter, 16-bit

| symbol | address | size | GOT slot |
|---|---|---|---|
| `DLWeightLightDubaiB16BIT` | `0x387cb4` | **582 580** | `0xd0d90` |

⚠️ The symbol size **582 580** is 145 645 words, not `0x238ed` (145 581). The
difference is **64 words (256 B)**. Do **not** conflate the two numbers: the
command body carries the **word count** (`+0xc3560`), while `memcpy` copies
`word_count × 4`, i.e. **582 324 bytes**.

### False-touch guard (`MisTouch`)

| symbol | address | size | used for |
|---|---|---|---|
| `DLWeightLight3966MisTouch_13_13` | `0x23cf5c` | 298 116 | `0x2c` |
| `DLWeightLightDubaiB4_3_0_0MisTouch` | `0x1f42d8` | 298 116 | `0x2e` |
| `DLWeightMis_3976ZS` | `0x285be0` | 298 116 | `0x30` |

### Calibration (`ASPDLWeight…PM`), 189 716 B

| symbol | address |
|---|---|
| `ASPDLWeightLight3956PM` | `0x2ce864` |
| `MistouchDLWeightLight3956PM` | `0x32b28c` |
| `ASPDLWeightLight3976PM` | `0x2fcd78` |
| `MistouchDLWeightLight3976PM` | `0x3597a0` |

✅ These four are **never called**: they have no GOT slots and no `adrp/add`
references in the image. They are build-time ballast, not part of any path.

## What is loaded for our chip (kind 0) ✅BIN

`hal_milan_select_model` starts at **`0x635c8`** (body `0x635c8…0x639fc`).

### Mechanics

```
63668: adrp x8, a9000
6366c: add  x8, x8, #0x6f4        → x8 = 0xa96f4   (TABLE START)
63670: ldr  x9, [sp, #40]         → w9 = chip − 0x16
63674: ldrsw x10, [x8, x9, lsl #2]
63678: add  x8, x10, x8           → target = 0xa96f4 + offset
6367c: br   x8
```

⚠️ Past attempts produced garbage because the target was computed as
`T + offset × 4`, and/or from the `add` address instead of the table base.
The correct form adds the offset **once**.

### Where each chip goes

| chip | target |
|---|---|
| `0x16` | `0x637a0` |
| `0x2b` (3626ZS1) | **`0x63680`** |
| `0x2c` (3636ZS1) | `0x63800` (error exit) |
| `0x2d` (3988) | `0x636e0` |
| **`0x2e` (3956 — ours)** | **`0x63680`** |
| `0x2f` (5658ZS3) | `0x63800` (error exit) |
| `0x30` (3976ZS1) | `0x63740` |

✅ Our chip `0x2e` uses branch **`0x63680`**, the one holding the word count
`0x238ed` = **145 581**.

### What the branch does

```
w8 = 0x0003d40 (250 688 — upper bound)
w9 = 0x000238ed (145 581 — word count)   → *word_count = w9
if w9 > w8 → 1003
x8 = pointer from the GOT
memcpy(destination, x8, w9 × 4)           = 582 324 bytes
```

✅ The GOT slot resolves to `R_AARCH64_GLOB_DAT … DLWeightLightDubaiB16BIT`,
the only symbol in the image with 145 581 words. So the stock stack loads
exactly the model our Rust loads.

## Summary

| kind | loaded for `0x2e` | words |
|---|---|---|
| **0** | **`DLWeightLightDubaiB16BIT`** | **145 581** |
| 1 | `DLWeightLightDubaiB4_3_0_0MisTouch` | 74 529 |
