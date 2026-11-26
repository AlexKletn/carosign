# Kernel and daemon — briefly

## `spinel_qseecom` — the `/dev/qseecom` shim ✅SRC

Translates ioctls of the stock library into `qcom_scm`. Accepts only
`0x6700…0x670b`, anything else gets `-ENOTTY`.

`qc_send`:
1. bounds (`cmd ≤ 64 KiB`, `resp ≤ 64 KiB`, `0 ≤ nfd ≤ 4`);
2. copy the command into tzmem;
3. for Goodix — raise `DLINA` (`+36`) to `0x4FFA8`;
4. bodies — from ion/dma-buf into tzmem, size = `max(fd size, declared length)`;
5. **the body address goes as a 32-bit word at the head of the header** (upper
   half zero), flags at `+4` untouched;
6. zero the reply before sending, `app_send`, then copy bodies and reply back.

## `spinel_fp_proba` — owner of the app ✅SRC

Steps: bind → memory → assemble the image → SHM bridge → start →
`load_keep` (power + IRQ + handed to the shim).

Start order: check `cmnlib64` → `app region` → library buffer →
`load_services` for `cmnlib`/`cmnlib64` → verify the app image → `app_start` →
`LOAD_KEEP` (wake, netlink, IRQ, `load_km`).

⚠️ **Only proba loads the app.** Rust never touches it; Rust only takes the id.

## Power and reset ✅SRC / ✅BIN

Stock driver (`gf_power_on` @`0xa709c8`, `gf_hw_reset` @`0xa70c48`,
`gf_power_off` @`0xa70bf8`):

```
power_on:  41 units → rail 3 000 000 µV → 15 → RST 1 → 25 → RST 0 → 6
hw_reset:  RST 1 → RST 0 → 50 880 → RST 1 → (ms more)
power_off: rail off → 1 unit
level:     RST is a RAW level (gpiod_set_raw_value), not "active low"
```

⚠️ The unit is a `__const_udelay` multiplier, **not** milliseconds: without
the stock `HZ` it cannot be converted. Order and ratios are proven; absolute
values are not.

⚠️ Our C (`spinel_fp_razbudit`, `spinel_fp_hw_reset`) uses a **different edge**
(`0 → 1 → 0`) and different delays — recorded as a discrepancy to be fixed and
measured at implementation time.

Stock device tree: RST — GPIO **85**, IRQ — **23**, rail **`pm8150a_l11`**
(`dtbo-02.dts`, `fragment@73`).

## `spinel_qsee_listener` + `spinel-lstnrd` ✅SRC

Three services: FS `0xa`, RPMB `0x2000`, GPFS `0x7000`. Buffers: FS and RPMB
32 KiB each, GPFS 504 KiB. The daemon works on **copies**; only the kernel
writes into the TZ buffer. The reply (`DONE(ret)`) must arrive within **5 s**,
otherwise the listener is poisoned until reboot.

State machine: `IDLE → WAITING → ANSWERED → IDLE`; a failure leads to
`POISONED`.
