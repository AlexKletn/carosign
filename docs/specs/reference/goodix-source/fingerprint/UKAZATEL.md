# Documentation index: fingerprint

Where to go when you are looking for an answer. Keep coming back here instead
of guessing paths.

## "How does it work?"

Start with **[`stok/README.md`](stok/README.md)** — the current specification
of the stock stack and of our C version.

| question | document |
|---|---|
| Layers, and who calls whom | `stok/00-overview.md` |
| What happens on open / close / restart | `stok/01-hal-lifecycle.md` |
| How a finger event arrives and what the reply holds | `stok/02-sensor-event.md` |
| Which commands are sent, with which bodies | `stok/03-commands.md` |
| What our modules and the daemon do | `stok/04-kernel-and-services.md` |
| What is still not understood | `stok/05-open-questions.md` |

## "I need a number"

**[`tablicy/README.md`](tablicy/README.md)** — commands, names (modes,
operations, navigation, errors, chips), weights, addresses, service contracts.

## "Why is it this way, and where did we go wrong?"

**[`stok/history/README.md`](stok/history/README.md)** — the 10–11 November
review sessions, checks 1–8, corrections to ourselves. Event measurements live
there too.

## "How was it before the reorganisation?"

**[`archive/README.md`](archive/README.md)** — the former `sistema/00–44`
layout with all the detail (decompilations, `libgf_ca`, power sequencing,
addresses). ⚠️ Part of it is outdated: `archive/README.md` has a table of
"what is superseded, and what replaced it".

## Living documents next to this index

* [`TZ-TRANSPORT-AUDIT.md`](TZ-TRANSPORT-AUDIT.md) — TrustZone measurement
  log (still growing).
* [`SLUSHATEL-PLAN.md`](SLUSHATEL-PLAN.md) — history and plan of the QSEE
  listener.
* [`GF-CALIBRATION-FORMAT.md`](GF-CALIBRATION-FORMAT.md) — layout of
  `gf_calibration.so`: a flat container with zlib CRC32 and `magic 0xF0…0xF7`.
  Reference only: in our path calibration is done by the TZ.
* For the stock stack:
  [`../../stock/fingerprint/RAZBOR.md`](../../stock/fingerprint/RAZBOR.md).

## House rules

1. **Current knowledge lives only in `stok/` and `tablicy/`.** The archive and
   the history are records of a search, not knowledge.
2. A new finding goes into `stok/` or `tablicy/` first; a narrative of the
   review session, if wanted, goes into `stok/history/` afterwards.
3. Provenance tags are mandatory: ✅BIN (binary, with an address), ✅SRC (our code),
   ✅MSR (measurement), ⚠️ (not understood). A note without them is not accepted.
4. "To be measured during implementation" is a separate tag — not a gap in the
   analysis.
