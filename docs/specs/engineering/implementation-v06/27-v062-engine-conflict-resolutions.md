# 27. v0.6.2 — B-01 and B-02: normative Engine, commit and error decisions

**Status:** binding decision supplement, 10.11.2026. Overrides any ambiguous “Committing or Terminal” suggestion in v0.6/0.6.1. All lower-priority v0.5 architectural diagrams are informational. The implementation agent must not invent a third path.

## B-01: independently verified reconciliation

Every affirmative reconciliation (post-state OR complete unchanged pre-state) enters `Committing`, never direct `Terminal`.

| Operation | Verified evidence | Metadata transaction in `Committing` | Terminal |
|---|---|---|---|
| `Enroll` | Desired new registration (authenticator, subject, identifier, generation all match) | Registry CAS insert/replace + journal/outbox | `Committed` |
| `Enroll` | Full original pre-state unchanged, including old registration in replace mode | Leave enrollments/index untouched; journal/outbox `MUTATION_NO_CHANGE` | `NoChange` |
| `Remove` | Target exactly absent; original target identity/generation verified | Registry CAS remove + journal/outbox | `Committed` |
| `Remove` | Exact original registration still present unchanged | Leave enrollments/index untouched; journal/outbox `MUTATION_NO_CHANGE` | `NoChange` |
| Either | Unknown, unverifiable, incomplete or mismatched | No CAS; scope lock retained | `NeedsReconciliation` |

`IndependentlyConfirmed` is replaced by typed `DesiredStateVerified` / `PreStateVerified` / `VerificationIndeterminate`. A simple `exists`/`absent` bit is not admissible: for replacement, absence of new record says nothing about old template preservation. Verified observations include scope (subject, authenticator, enrollment IDs, epoch/generation), source evidence and the actual pre-/post-state. The trust policy must independently approve the provenance. If no independent hardware method exists (e.g., Goodix baseline), state remains blocked.

**Atomicity:** both verified paths commit metadata (journal/outbox and optional Registry CAS) durably, retain the scope lock until quiescence, and then finalize `MUTATION_COMPLETED` plus `terminal_outcome` and lock release in one transaction. For NoChange, the metadata transaction validates the expected Registry baseline under scope lock, even though enrollment rows are not changed. `MUTATION_NO_CHANGE` is a nonterminal metadata stage; a crash before cleanup does not yield terminal success. `MUTATION_COMPLETED` alone cannot recover which outcome occurred. `storage.proto` now has explicit enum `MutationTerminalOutcome` and field `MutationRecord.terminal_outcome = 14`; on completed record it MUST be Committed/NoChange, on other phases MUST remain UNSPECIFIED. Missing outcome in a hypothetical deployed completed record requires explicit migration; never default to Committed.

## B-02: metadata and verify dispatch failures

**Mutation `MetadataFailed` after arm**: always `Reconciling` + quarantine; no hardware resend. A commit error/timeout can mean *either* no commit or durable commit with lost ACK. Upon recovery read actual DB phase/Registry state and validate independently before finalizing. When DB itself unavailable and `mark_uncertain` cannot be persisted, Runtime enters local `StorageUnavailable` and blocks affected scope/any dependent service flow; it must not claim a durable reconciliation marker was written. This condition survives restart through the already-durable arm/outbox or committed metadata when storage becomes readable. Corrupt records stay fail-closed.

**Verification send**: `SendFailedBeforeArm` is invalid terminology since Verify has no persistent arm. If trustworthy local effect-runner receipt proves *not sent*, raise `VerifyDispatchNotSent`, terminal internally `Failed(DispatchNotSent(reason))`; public may display `Unavailable` only when underlying reason denotes transport/device outage. If send may have reached worker but ACK was lost, raise `VerifyDispatchUnknown`, suppress potential late match, request cancel/quiescence, and mark resource Uncertain until confirmed quiescence. An unsupported operation or denied grant is not an availability failure. Neither path yields `NotMatched` or success.

## Actor + storage responsibilities

- Engine operates on typed, prevalidated observations. `MetadataFailed`, `VerifyDispatchNotSent` and `VerifyDispatchUnknown` never originate unchecked from worker bytes.
- Persistence returns durable receipts only after the transaction's documented durability; failed/unknown receipts require reread rather than blind retry.
- `MutationRepository::commit_registry` takes validated `CommitEnrollment::{ApplyDesiredState,RecordVerifiedNoChange}` (discriminated enum), not a bool to skip writes.
- The terminal outcome is durably stored and public result is emitted **only** after safe cleanup, terminal journal commit, current owner authorization and compatible evidence rules.
- Resource cleanup failure can retain the scope lock and block public terminal delivery even when metadata change already committed.

## Acceptance gates

Conformance tests: `B-01a..B-01f`, `B-02a..B-02f` in [15](15-conformance-matrix.md). An agent that encounters different actual runtime or backend semantics must raise `ARCHITECTURE_BLOCKER`, not choose an alternative.
