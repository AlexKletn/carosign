# 05 — What is still open, and how it is closed

State on 11 November 2026, after the reorganisation.

## Gaps

| # | gap | state |
|---|---|---|
| 1 | which weight symbol is used for chip `0x2e` (kind 0) | ✅ **closed**: `0x2e` → branch `0x63680`, `0x238ed` words = `DLWeightLightDubaiB16BIT` |
| 2 | why the direct path sees no second event after the first touch | ⚠️ **open**; code analysis showed no difference — needs a measurement during implementation |
| 3 | `ASPDLWeight…PM` — where they are used | ✅ **closed**: present in the image, called by nobody |
| 4 | format of `asp_template_*` | ✅ **closed at its boundary**: owned by the TZ, the HAL does not parse it |
| 5 | `libgf_ca`: key, AES, digest | ✅ **not required** for our path (whitebox disabled) |
| 6 | absolute milliseconds of power-on and reset | ✅ **a property of the stock code**, not a gap |

## What this means

Nothing blocks the start of implementation except gap 2 — and by agreement it
is closed by a measurement on the device, once implementation begins.

## Where to look when implementing

1. Order and bodies — [01](01-hal-lifecycle.md) and [03](03-commands.md).
2. Events — [02](02-sensor-event.md).
3. Kernel and power — [04](04-kernel-and-services.md).
4. Numbers — [tablicy/](../tablicy/README.md).

## What was already measured (for cross-checking, do not repeat blindly)

✅MSR Gatekeeper `enroll`/`verify`, template enrolment, matching, the Rust
listener service, the direct path `DETECT_SENSOR`/`INIT`/`INIT_FINISHED`/`1111`,
the stock HAL as yardstick (`auth_ok`, `match_score=35`), and zero IRQ edges on
an idle sensor for 90 s. The template hash `9da2e9cb…` was unchanged by every
run.
