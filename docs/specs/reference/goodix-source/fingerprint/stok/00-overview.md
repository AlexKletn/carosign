# 00 — Overview: layers, and who calls whom

## Layers

```
  stock (Android)                        our path (Linux)
  ──────────────────────────────────     ─────────────────────────────
  libgf_hal.so   HAL orchestration       (ported to Rust:
                 1000…1111, mask/         goodix.rs, irq.rs,
                 operation, notices       authentication.rs)
  libgf_ca.so    channel to the TZ app   —
  libQSEEComAPI  ioctl /dev/qseecom      spinel_qseecom (shim)
  libion         memory for command      /dev/dma_heap/system (Rust)
                 bodies
  libdrmfs.so    FS service              spinel-lstnrd + spinel_qsee_listener
  librpmb.so     protected RPMB writes   spinel-lstnrd (SG_IO)
  —              GPFS 0x7000             spinel-lstnrd
  Android HAL    —                       spinel_goodixfp (stub)
  kernel driver  power/reset/IRQ         spinel_fp_proba (owner)
  QSEE kernel    qcom_scm                qcom_scm + our patches
  TZ: goodixfp, keymaster64, cmnlib(64)
```

## Two roads to the sensor

**1. Direct (ours).** Rust opens `/dev/qseecom`, issues `LOAD_APP goodixfp`,
and the shim returns the id that proba already loaded. The command body goes
into DMA memory, and its physical address goes into the head of the command
header. Events do **not** come from the TZ: `spinel_fp_proba` holds the sensor
IRQ line and sends a netlink message on unit 29.

**2. Stock (the yardstick).** `libgf_hal.so` → `libgf_ca` → `libQSEEComAPI` →
`/dev/qseecom` → the same road beyond that. It produces `auth_ok` on the
tablet ✅MSR, and serves as the measure of "what it must do".

## Who is responsible for what

* **proba** — the owner: loads `goodixfp` and `keymaster64`, powers the sensor,
  holds the IRQ, broadcasts events. It, and only it, loads the app.
* **shim** — translates stock ioctls into `qcom_scm`, keeps buffers in tzmem.
* **listener + daemon** — serve TZ requests for files (FS), storage (GPFS) and
  RPMB.
* **HAL** — orchestration: which commands, in what order, how replies are read.
* **our Rust** — the same as the HAL, but without Android: commands, events,
  matching.
