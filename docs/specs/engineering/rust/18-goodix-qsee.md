# 18. `carosign-provider-goodix-qsee`: Rust worker internals

Proposal for a product implementation of TB371FC Goodix; previous C implementation is a **reference prototype**, not code that must be mechanically ported. Current source material documents successful stock HAL path and remaining direct native IRQ-path uncertainty; do not claim this Rust worker is already complete.

## Separate workspace

```text
providers/goodix-qsee/
  crates/
    qsee-transport/          # scoped interface to Linux QSEE system
    qsee-listener-client/    # control/availability to listener owner
    goodix-wire/            # binary protocol, exact lengths/status
    goodix-device/          # power/reset/IRQ and lifecycle
    goodix-ta/              # trustlet commands/sessions
    goodix-orchestrator/    # HAL-equivalent state machine
    goodix-provider/        # normative provider protocol mapping
  bins/
    bio-provider-goodix-qsee/
```

## Low-level modules

- `qsee-transport`: `session.rs`, `command.rs`, `dma.rs`, `ioctl.rs`, `scm.rs`, `errors.rs`; only restricted broker-granted operations, no public raw TA invoke.
- `qsee-listener-client`: `registration.rs`, `health.rs`, `fs.rs`, `gpfs.rs`, `rpmb.rs`, `poisoned.rs` (client-facing state/coordination; actual listener may be separate long-running privileged process).
- `goodix-wire`: `header.rs`, `command_id.rs`, `encode.rs`, `decode.rs`, `status.rs` (transport vs TA status), `offsets.rs`, `fixtures.rs`. No unbounded deserialization, sizes/offsets validated bytewise.
- `goodix-device`: `power.rs`, `reset.rs`, `irq.rs`, `clock.rs`, `probe.rs`, `state.rs`. Kernel-driver dependencies via scoped Linux adapter.
- `goodix-ta`: `load.rs`, `open.rs`, `invoke.rs`, `event.rs`, `template.rs`, `session.rs`, `recovery.rs`.
- `goodix-orchestrator`: `init.rs`, `sensor_events.rs`, `enroll.rs`, `verify.rs`, `remove.rs`, `cancel.rs`, `persist.rs`, `fsm.rs`; contains known HAL-equivalent sequencing while core remains unaware of Goodix command numbers.
- `goodix-provider`: `worker.rs`, `profile.rs`, `capabilities.rs`, `result_mapping.rs`, `session_actor.rs`, `reconcile.rs`, `faults.rs`.

## Invariants and device-specific constraints

- Recognize both transport status and TA status; a successful transport call is not successful fingerprint operation.
- TA listener health and storage callbacks have independent lifetime. `POISONED` cannot be treated as simple retryable return.
- Provider-owned templates remain opaque; platform registry owns only binding and recovery reference.
- Missing second IRQ/direct event path means native verification still needs engineering investigation; common cancellation/recovery cannot be used to mask missing hardware behavior.
- `SAVE_TEMPLATE` and related commands are mutating; after possible dispatch, crash → journal reconciliation, never blind re-send.
- TEE execution alone does not prove hardware token is accepted by KeyMint or protected recipient.

## Safety/isolation

Prefer separate listener owner and QSEE broker when raw interface grants excessive rights. Test full path with controlled test identities and no unplanned RPMB modifications. Profile-specific firmware/TA dependencies are packaged separately from core.

## Acceptance

Cold boot → bind device → enroll (real) → persist/reboot → verify same/other → remove → reconcile interrupted mutation → clean teardown; protocol version and package upgrade independent of carosignd.
