# 17. Differences from v0.5 and verified-unknown boundary

## Normative upgrades

| v0.5 | v0.6 baseline |
|---|---|
| Internal Rust signatures illustrative | Enumerated owner modules, exact domain contracts and work orders |
| Provider proto sketch has `bytes context` and `string kind` placeholders | Dedicated typed `.proto` message schemas in `contracts/proto` |
| Persistence `redb proposed` | Select redb as initial metadata backend, versioned value format, journal+outbox in one transaction domain |
| Mutation barrier abstract | Durable `DispatchArmed` **before** IPC send; crash after arm may be indeterminate even if send did not happen |
| Cancel semantics general | Control-plane ACK separate from confirmed hardware quiesce; no boolean cancel completion |
| Device matching conceptual | Explicit locator/identity/authenticator separation and ambiguity fail-close |
| Language neutrality principle | Exact independent worker ABI, GLib bridge conformance, no Rust runtime dependency |
| Architecture not complete for implementer | Numbered W00..W15 tasks, black-box matrix, review gates |

## Evidence gaps not filled by design

**Goodix/TB371FC:** Device-specific QSEE/TA commands, listener semantics and IRQ details exist in research archive. This pack does not re-verify binaries or substitute new guessed commands. Exact independent TA query for recovery may not exist: preserve `NeedsReconciliation` until established.

**GXFP5130:** Host matcher threshold values from existing project are experimental, not certified FAR/FRR. No blanket hardware protection. Existing PSK/keyfile/TPM usage must be re-audited before production.

**Nord 4/Veles:** Current trust-anchor integration research includes synthetic tests and partial real QTEE work. Complete trusted fingerprint-token→KeyMint operation and trusted input/UI chain are **not yet confirmed**. Never assert hardware strict assurance from this file alone.

**libfprint:** Actual API availability and FpPrint serialization compatibility vary with library/device versions. Bridge must feature-detect; unsupported operation returns Unsupported; no device auto-migration based on matching chip name.

## STOP examples

- Claim "TA supports query enrollment ID" without recorded hardware evidence → STOP.
- Need Root access in provider because ioctl requires privilege → STOP; evaluate broker mediated call, not raw root.
- Local database cannot honor durable commit semantics → STOP; do not allow mutation dispatch.
- Hardware token cannot be verified independently → STOP for protected operations, software-only policy may still be available when separately authorized.
- C ABI cannot express a feature → update `.proto`/SDK through ADR, **not** add hidden Rust-only method.

## Source preservation

v0.5 docs and user-provided `reference/` remain included as historical/context material. v0.6 additions have precedence for implementation baseline; actual research observations in source files are not rewritten as if already independently verified by v0.6.

### v0.6.1 resolution for Goodix (not new hardware evidence)

Goodix/TB371FC has **no established safe independent reconciliation oracle** in supplied documents. Baseline `QueryMutation` returns `QUERY_UNSUPPORTED` when capability absent, `QUERY_UNKNOWN` for inconclusive attempted supported query; both block affected scope and affected verification use. A verified safe TA query or reset/reprovision+independent confirmation may be added only after device research and accepted ADR. These rules are a safety policy, **not a claim that any reset or query exists**. See [25](25-v061-conflict-resolutions.md).

### v0.6.5 evidence boundary

B-10 HAT and B-11…B-13 large command body are now defined at **generic** architectural boundaries (31/32) but remain unqualified for actual Goodix production until trusted Gatekeeper issuance/sink, kernel/QSEE typed handlers, DMA lifetime and hardware behavior are established on device. A logical v2 MutationPlan (33) makes restoration deterministic without inventing a physical TA oracle. No test or reset command has been synthesized. The reconstruction of the missing `27-enrollment-challenge-gap.md` is explicitly labeled, not presented as the original. See [30](30-v065-audit-and-authority.md).
