# Fingerprint driver and HAL: documentation tree

The fingerprint sensor on TB371FC is described in three layers. Do not mix them.

| folder | what it is | who it is for |
|---|---|---|
| **[`stok/`](stok/README.md)** | **current specification** — what the stock stack and our C version do | anyone asking "how does it work" |
| **[`tablicy/`](tablicy/README.md)** | reference tables (names, command bodies, weights, addresses) | when a number is needed, not a story |
| **[`stok/history/`](stok/history/README.md)** | how we got here: review sessions, mistakes, corrections | so the mistakes are not repeated |
| **[`archive/`](archive/README.md)** | the former `sistema/00–44` layout, before this reorganisation | for cross-checking the 10–11 November notes |

## Where to start

1. **[`stok/00-overview.md`](stok/00-overview.md)** — layers, and who calls whom.
2. **[`stok/01-hal-lifecycle.md`](stok/01-hal-lifecycle.md)** — open, run,
   close, restart.
3. **[`stok/02-sensor-event.md`](stok/02-sensor-event.md)** — the subtlest part:
   finger events, mask, operation, steps.
4. **[`stok/03-commands.md`](stok/03-commands.md)** — what the HAL sends and in
   which order.
5. **[`stok/04-kernel-and-services.md`](stok/04-kernel-and-services.md)** — our
   kernel modules and services.

## House rules for this folder

⚠️ These documents were reorganised on 11 November 2026. Papers written during
the 10–11 November review sessions repeat each other and contain corrections to
themselves. To keep them from being read as current knowledge, every such
document carries "⚠️ **Superseded: see …**" on its first line:

* [Model weights, kind 0](stok/history/02-model-weights-search.md)
* [Jump tables](stok/history/03-jump-tables-first-try.md)
* [File vs image addresses](stok/history/04-file-and-image-addresses.md)

✅ Only `stok/` and `tablicy/` carry current knowledge.
