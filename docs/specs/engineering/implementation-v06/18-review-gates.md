# 18. Review gates, required reports and acceptance discipline

## Gate G1 — Core API stability

Public domain values and constructors fixed, no leakage of OS identifiers. Running pure tests demonstrates invariants. Review requires diff against [03](03-domain-types.md) and explicit sign-off on any public type change.

## Gate G2 — Effect and durable safety

Per-operation state deterministic, no hardware action before durable `DispatchArmed`, atomic metadata CAS+journal, tested kill windows. Accept only with evidence from fault matrix D-01..D-08; simulated hardware still labeled simulation.

## Gate G3 — Wire interoperability

Generated Protobuf from checked-in `.proto`, no placeholder fields; all IDs length-checked; malformed payloads negative tested; independent non-Rust implementation (minimum C reference) matches traces. Protocol schema change requires version/compatibility review.

## Gate G4 — Privilege boundary

Worker separately spawned, restricted resource broker; unrelated device access denied; FD leaks and unexpected descriptors rejected. No network by default. Worker's manifest not authority. Security-sensitive platform calls use explicit trust policy.

## Gate G5 — End-to-end simulated product

Zero providers ready, one simulated provider full verify/enroll/remove, live cancellation, crash after mutation arm safely quarantined, PAM fallback documented. No real-device security claim.

## Gate G6 — Hardware provider acceptance

Independent install, real sensor success and negative tests, safety after reconnect/reboot, template durability, no confidential debug logs, explicit risk classification and available recovery. Hardware logs identify actual kernel/build/TA/profile and dates. Gate is per provider/device profile, not a blanket platform guarantee.

## Gate G7 — Veles integration

Demonstrate same provider wire via external adapter, delegated trusted grants, independent protected recipient verification, real trusted input chain before declaring strict class. No Veles source dependency inside platform crates.

## Required report format for every task

```
WorkOrder: Wxx
Changed: <paths>
Implemented: <exact contract items>
TestsExecuted: <commands and outcomes; NOT RUN if unavailable>
Conformance: <IDs passed / failed / not run>
SafetyReview: <impact on grants, proofs, mutation, FD/IPC>
Assumptions: <none, or explicitly documented non-security local detail>
ArchitectureBlockers: <none, or ARCHITECTURE_BLOCKER records>
NextAllowedWorkOrder: <ID>
```

**Stop-the-line:** never invent architecture or hardware claims to make tests pass. No acceptance of "it works" without required negative/crash tests for security-critical modules. If tools are unavailable, mark not run, do not claim validation.

## v0.6.5 additional release gates

G3 now requires provider minor 2 + broker minor 1 descriptor/negative tests, including optional sealed-memfd semantics, with independently generated C/Rust wire fixtures. G2 now requires `MutationRecord.plan` schema 2 and no replay of armed mutation without an independent physical witness. G4 requires credential authority HAT binding (H-01…10), broker sealed-fd faults (L-01…14), strict StartRemoval scope S-01, and no generic raw QSEE. G6 additionally requires actual typed kernel/TA handler behavior and safe DMA lifetime; no production enrollment claim from mere HAT acceptance. If compiler/hardware not run, report NOT RUN; documented tests are not tests passed.

## v0.6.6 command-projection release gates

Supersede the v0.6.5 provider/minor and storage/schema numbers: G3 requires provider minor **3**, broker minor **1**, with `planned_enrollment` validation and independent C/Rust wire fixtures; G2 requires stored schema **3**, exact selected old-target and per-binding custody kind in canonical digest; G4 rejects any generic fallback to unscoped binding; G5/G6 include B14-01..12 from [34](34-v066-enrollment-command-projection.md). Existing H/L/R/S scenarios remain mandatory. No test is considered passed merely by writing the fixture definition.
