# `gf_calibration.so` format (TZ trustlet `goodixfp`, SM8250) — analysis of 3 September 2026

The container is flat: blocks of "1 magic byte + payload". There is **no**
signature or HMAC — only a zlib CRC32. Decoder: `gf_cali_common_load` @`0x5bb28`;
`check_crc32` @`0x5b808`. `N = width × height` (`int16`); the geometry comes from
the runtime config word @`0x689ef8` (bits 14..22 and 23..31 — the two sides).

| off | size | field | checked |
|---|---|---|---|
| 0 | 1 | magic `0xF0` | yes |
| 1 | 4 | CRC32 (BIG-endian) | yes |
| 5 | 1 | magic `0xF1` | yes |
| 6 | 64 | `preprocess_version` (string) | first 20 bytes |
| 70 | 1 | magic `0xF2` | yes |
| 71 | 1 | magic `0xF3` | yes |
| 72 | 1 | magic `0xF4` | yes |
| 73 | 1 | magic `0xF5` | yes |
| 74 | 1 | magic `0xF6` | yes |
| 75 | 2N | base frame, `int16[N]` | length only |
| 75+2N | 1 | magic `0xF7` | yes |
| 76+2N | 2N | b/ori_b frame, `int16[N]` | length only |

Total: `76 + 4N` bytes.

CRC32: standard zlib (poly `0xEDB88320` reflected, init `0xFFFFFFFF`, xorout
`0xFFFFFFFF`), covering the buffer from byte 5 to EOF (the whole blob minus the
`0xF0` and the 4 CRC bytes), stored BIG-endian in bytes 1..4. Table @`0x427fa0`.

`preprocess_version` is EXPECTED to be `"Preprocess_v_1.01.01"` (@`0x425398`),
compared over 20 bytes. A mismatch → `1048`
(`GF_ERROR_INVALID_PREPROCESS_VERSION`). `frameNum`: global range check
`[1,5]`. `sensor_id`: 16 bytes, `memcmp` against @`0x689f24` (filled from the
sensor at runtime). Bad magic or CRC → `1022`; bad params or length → `1003`;
sensor-id mismatch → non-zero.

The ONLY runtime unknowns for forging are `sensor_id` (16 bytes, equality only —
not crypto, so any known value such as `GF9558` will do) and the geometry `N`.
The decoder ACCEPTS zeroed base/b frames (`memcpy` with no arithmetic).
`chip_id`/`vendor_id` are not checked.
