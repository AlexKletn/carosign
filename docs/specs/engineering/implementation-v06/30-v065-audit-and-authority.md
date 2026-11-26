# 30. v0.6.5 — independent specification audit and decision precedence

**Status:** normative correction set for v0.6.5 (11 November 2026). This document plus 31–33 and updated `.proto` supersede conflicting earlier wording. It does not assert successful Rust implementation, deployed schemas, independent C interoperability, or secure hardware validation.

## Findings

| ID | Severity | Finding in v0.6.4 | Resolution/status |
|---|---|---|---|
| B-10 | Blocking | No enrollment challenge/HAT path; `replace_binding` only | 31, provider v1.2, capability field; generic contract resolved, hardware adapter evidence open |
| B-11 | Blocking | No way to deliver >=64KiB typed broker command bodies | 32, broker v1.1, sealed memfd; hardware-handler implementation open |
| B-12 | Blocking | Buffer ownership, size, length, mapping, cache sync and DMA32 not normative | 32, bounded memory chain; exact kernel UAPI to be verified per hardware |
| B-13 | Blocking | No failure/retry/cancel semantics for large in-flight buffer through module/TA | 32, full lifetime and fault handling; hardware fault injection open |
| A-15 | Blocking | Persistent canonical mutation plan absent despite Engine requirement | 33, storage schema v2 and immutable logical plan; independent physical witness remains capability-dependent |
| A-16 | Major | Provider protocol v1.1 clashed with v1.0 sample manifests and traces | Package samples/traces/validator now v1.2; older text labelled historic |
| A-17 | Major | Broker forbidden ancillary FD despite large hardware payload research | Explicitly restricted broker-v1.1 exception, without weakening provider-control FD rule |
| A-18 | Major | Draft docs might be read as globally requiring host template vault or generic matcher | 29 retains sole authority; 31–33 do not add any central biometric processing |
| A-19 | Major | HAT acceptance and generic manage grant could be conflated | Trust authority separation, token issuance/recipient binding and refusal rules in 31 |
| A-20 | Major | Reconciliation snapshots implied that a logical DB record proves physical state | 33 distinguishes durable comparison plan from independently verified hardware witness |
| A-21 | Medium | Competing numbering `27` could overwrite engine-conflict resolution | gap note saved as `27-enrollment-challenge-gap.md`, engine resolution untouched |
| A-22 | Medium | v0.6.4 README/VALIDATION and stale comments can be read as current baseline | v0.6.5 index/README, explicit precedence and checks |

## Precedence / implementation boundary

1. New normative [31](31-v065-enrollment-authorization.md), [32](32-v065-bulk-command-buffer.md), [33](33-v065-durable-mutation-plan.md), updated `provider.proto`, `broker.proto`, `common.proto`, `storage.proto` take priority for corresponding subject matter.
2. [29](29-v064-provider-owned-matching.md) remains authoritative for **all** capture, features, matching, score thresholds and carosign-template algorithms: worker/provider only. No shared host matcher introduced.
3. [28](28-v063-operation-discriminant-and-send.md) continues to govern discriminated machine and typed send receipts. [27-v062](27-v062-engine-conflict-resolutions.md) continues to govern reconciliation and metadata effects.
4. `reference/` is evidence/research only, not implementation normative. A hardware-specific statement in these documents requires an evidence-tagged provider ADR before being coded into a trusted broker handler.
5. Legacy illustrative code/protobuf snippets in `protocols/` or `engineering/rust/` are non-normative whenever they conflict with checked-in `contracts/proto` and implementation-v06/30–33.

## Historical source-link debt (outside normative scope)

The imported `reference/**` research collection retains **14 inherited broken relative Markdown links** to external or absent project trees (including `concept/`, a local workstation path and archived stock research). They were **not** rewritten to invented targets or deleted because those files preserve provenance. All Markdown links **outside `reference/**` resolve** within this package. Restoring the reference source tree or replacing each link with a verified citation is a separate documentation task, not an implementation contract for Rust.

## Unresolved evidence and gates (do not mark complete)

- **Goodix:** safe independent query / physical-state reconciliation still unknown; `NeedsReconciliation` remains. The observed Goodix enrollment ended with storage error 1035, **not** registration success. Full trusted hardware command path and verified kernel module build/ABI must be demonstrated separately.
- **HAT:** real credential authority, challenge provenance and broker's secure injection handler are not supplied by this generic spec. If these are absent, HAT-required enrollment is unavailable; no test-bypass fallback.
- **Large bodies:** generic mediated memory and fault contract is specified; safe handler-specific codecs/TA framing, exact ioctl ABI, kernel patch/version and DMA assumptions need device tests. Unsupported adapters return `UNSUPPORTED` before mutating effects.
- **Reconciliation:** canonical *logical* plan encoding is fixed; it does not provide an independent hardware oracle or permit claiming `PreStateVerified` from Registry only.
- **Vault:** scoped vault IPC/atomic staging remains separate STOP/ADR, not filled by a generic command buffer.
- **Protocol:** protoc-generated descriptor-set and independent C/Rust conformance have **not** been claimed by this package until actually executed.

## Implementation agent rule

A missing hardware proof or unresolved trust-port integration is **not** permission to add a convenience fallback in Runtime, leak HAT in public IPC, make arbitrary broker calls, pretend a query exists or silently downgrade to non-HAT enrollment. Stop in the relevant provider adapter and report required device evidence.

## Additional finding in v0.6.6

**B-14 (blocking implementation contract):** v0.6.5 persisted `transition` but did not define the trusted construction of `MutationChange::Enrollment` / `CommandPlan`; implementing both old/new values as absent loses the create/replace distinction. v2 snapshots also lacked custody kind and selected `old_target` in the canonical digest; the Registry enrollment record lacked a persisted kind; provider 1.2 had no field for the system-assigned future enrollment identity. **Resolution:** [34](34-v066-enrollment-command-projection.md), storage schema v3, provider protocol v1.3. This does not close any independent hardware evidence gate.
