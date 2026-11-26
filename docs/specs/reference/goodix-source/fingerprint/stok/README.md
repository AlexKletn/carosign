# Stock stack and our C version: how it works

Current specification. Everything here is either read from a binary (✅BIN,
with an address), read from our code (✅SRC), or measured (✅MSR). Where that is
not so, it says ⚠️ and "not understood" — no guesswork by similarity.

## Overview

| # | document | subject |
|---|---|---|
| 00 | [Overview: layers, and who calls whom](00-overview.md) | the whole picture, two roads to the sensor |
| 01 | [HAL lifecycle](01-hal-lifecycle.md) | open, close, restart, enrolment, matching |
| 02 | [Sensor event](02-sensor-event.md) | `1016`, mask, operation, operator, steps |
| 03 | [Commands and their bodies](03-commands.md) | list, bodies, order, attempt limits |
| 04 | [Kernel and services](04-kernel-and-services.md) | shim, proba, listener, daemon, stock kernel driver |
| 05 | [What is still open](05-open-questions.md) | gaps, their state, how each is closed |

## Reference tables

| document | subject |
|---|---|
| [tablicy/commands.md](../tablicy/commands.md) | all commands, numbers and bodies |
| [tablicy/names.md](../tablicy/names.md) | modes, operations, navigation, errors, chips |
| [tablicy/weights.md](../tablicy/weights.md) | weight symbols, sizes, slots |
| [tablicy/addresses.md](../tablicy/addresses.md) | stock function addresses, and how to read them |
| [tablicy/service-contracts.md](../tablicy/service-contracts.md) | FS, GPFS and RPMB contracts |
| [tablicy/kernel-and-daemon.md](../tablicy/kernel-and-daemon.md) | our modules and the daemon, briefly |
| [tablicy/frame-header.md](../tablicy/frame-header.md) | command header, reply and event-body offsets |

## Earlier documents (not rewritten, but important)

* [`../TZ-TRANSPORT-AUDIT.md`](../TZ-TRANSPORT-AUDIT.md) — TrustZone transport
  measurement log (September–November). Living, still growing.
* [`../SLUSHATEL-PLAN.md`](../SLUSHATEL-PLAN.md) — plan and history of the QSEE
  listener.
* [`../GF-CALIBRATION-FORMAT.md`](../GF-CALIBRATION-FORMAT.md) — calibration
  container format.
* [`../../stock/fingerprint/RAZBOR.md`](../../../stock/fingerprint/RAZBOR.md)
  — the stock analysis of 1 September (3 000 lines), with addresses.
