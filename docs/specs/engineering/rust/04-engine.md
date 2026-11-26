# 04. `carosign-engine`: чистые state machines

**Goal:** separate decisions from execution; deterministic transitions reproducible after failure. `#![forbid(unsafe_code)]`, no executor, sockets, timers or DB.

## Modules

```text
src/
  lib.rs
  input.rs                   # Observed external events
  effect.rs                  # Declarative effects executed by runtime
  transition.rs              # transition(state, input) -> decision
  invariant.rs               # state consistency and fencing
  machines/
    verification.rs
    enrollment.rs
    removal.rs
    worker.rs
    device.rs
  reconciliation.rs          # classify interrupted mutation
  replay.rs                  # restore state from snapshots/events
  test_vectors.rs            # optional feature for conformance fixtures
```

## Core API sketch

```rust
pub struct Decision<S> { pub next: S, pub effects: Vec<Effect> }
pub fn transition(state: &OperationState, event: DomainEvent) -> Result<Decision<OperationState>, TransitionError>;
```

`Effect` examples: `RequestAuthorization`, `AcquireResourceSet`, `DispatchProviderCommand`, `PersistIntent`, `CommitRegistry`, `VerifyEvidence`, `RequestQuiesce`, `DeliverOutcome`. Engine **requests** an effect; runtime decides when and how to execute and feeds back explicit observations (`IntentDurable`, `CommandSent`, `ProviderOutcomeObserved`, `LeaseReleased`, etc.).

## Critical rules

- `BeginEnrollment` cannot produce `DispatchMutatingCommand` until observing `IntentDurable` and required authorization/revalidation.
- Once `CommandSent` is observed for mutation-capable command, `WorkerDisconnected` => `NeedsReconciliation`, not `NoChange`.
- `CancelRequested` changes intent/control state; it does not emit terminal success until `Quiesced` or `BeforeMutationConfirmed`.
- `Matched` provider observation enters `Finalizing`, never directly the public success; evidence/authority results are separately observed.
- Stale worker generation or operation id leads to an ignored/rejected observation, never a transition.
- Internal side-effect ordering defined via effect barriers: persist → dispatch → observe → commit → acknowledge.

## State separation

Three machines must not be collapsed: `OperationMachine`, `WorkerMachine`, `DeviceMachine`. `EnrollmentMachine` is a subtype of operation with persistent metadata; `WorkerMachine` may be `Ready` while device is `Uncertain`.

## Replay limits

Pure replay reconstructs intended state; it does **not** prove whether a TEE mutation happened. Recovery from storage produces `NeedsReconciliation` if any required evidence is absent. The runtime must consult an independent provider query/verifier before declaring consistent.

## Testing

Property tests for no double dispatch, terminal outcome uniqueness, stale event rejection and monotonic epochs. Model checking of conflicting transitions is a recommended later extension. Test deterministic traces stored as fixtures to compare implementation changes.
