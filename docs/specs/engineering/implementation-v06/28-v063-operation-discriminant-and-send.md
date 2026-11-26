# 28. v0.6.3 — OperationState discriminant and dispatch isolation (C-04 / E-08..E-14)

**Status: normative implementation baseline, 11.11.2026.** Overrides ambiguous or generic dispatch wording in 04/22; preserves 25–27. Provider wire / persisted protobuf version is **not** changed by this clarification. The machine's internal state is not a public Rust ABI.

## C-04: the operation machine is a tagged sum type

The previous phrase “`OperationState` includes operation kind” was not an explicit layout contract. **`state.mutation.is_some()` MUST NOT be used to select a state machine.** Neither may the caller supply a mutable `kind` independent of the machine variant.

```rust
pub struct OperationState {
    common: CommonOperationState,
    body: OperationBody,
}

pub enum OperationBody {
    Verification(VerificationState),
    Enrollment { request: EnrollRequest, state: MutationState },
    Removal { request: RemoveRequest, state: MutationState },
}

impl OperationState {
    pub fn kind(&self) -> OperationKind {
        match &self.body {
            OperationBody::Verification(_) => OperationKind::Verify,
            OperationBody::Enrollment { .. } => OperationKind::Enroll,
            OperationBody::Removal { .. } => OperationKind::Remove,
        }
    }
}
```

`CommonOperationState` contains the validated immutable `OperationContext`, transition index, worker/device generation binding, cancellation flag, pending effects, and optional outcome **only**. Machine-specific phase and mutation state live in the corresponding `OperationBody`. The internal exact field types that are already fixed in 04/05/22 are reused, not duplicated. All discriminants and fields are private outside Engine; expose checked constructors `new_verification`, `new_enrollment`, `new_removal` and read-only accessors. Do not add an independent `kind` field to the state: `OperationBody` is the single discriminant; `OperationKind` from Core is a derived view. `EnrollRequest.replace: Option<EnrollmentRef>` distinguishes create from replace; replacement is still `OperationKind::Enroll`, not an independent machine.

`decide()` matches **only** on `OperationBody` and dispatches to one of `verification::decide`, `enrollment::decide`, `removal::decide`. An input from the wrong machine yields `Err(TransitionError::InvalidInputForOperation)` and **no** next state, effect, or index increment. No fallthrough, no coercion, no `is_mutation_request`. A malformed restoration snapshot MUST fail closed. `MutationRecord.kind` remains `ENROLL|REMOVE` in storage; it is checked against the tag during mutation restoration, but is not the discriminator at runtime. Verification is never resurrected from a mutation journal.

### Construction/recovery

- `new_verification` has no mutation field and no mutation effects.
- `new_enrollment`/`new_removal` require a complete validated, immutable mutation plan at creation, including the scoped precondition and postcondition required by reconciliation, not merely an optional presence flag.
- Rebuilding after restart must validate the operation's class against `MutationRecord.kind`, expected subject/authenticator, epoch and original plan. Do not infer class from record presence or a missing field.
- If the complete trusted **pre/desired** comparison context cannot be recovered, `ReconciliationVerdict::Indeterminate` is mandatory. Persisting a full canonical plan for every supported backend requires a separately specified storage representation; until its schema is fixed, an implementation requiring this after reboot reports `ARCHITECTURE_BLOCKER` rather than inventing bytes or treating `expected_generation` as a full snapshot.

## C-05: typed send effects, receipts and domain inputs

`EffectSpec::SendProvider` and generic `SendAccepted` / `SendFailed` SHALL NOT route an operation. The producer of each trusted observation is the **effect runner**, not the worker's raw frame.

| Machine | Effect | Valid local result | Semantic result |
|---|---|---|---|
| Verification | `SendVerify` | `VerifyDispatchAccepted` | Worker accepted command; not a matched result |
| Verification | `SendVerify` | `VerifyDispatchNotSent` | Validated proof that no dispatch occurred; `Failed(DispatchNotSent)` |
| Verification | `SendVerify` | `VerifyDispatchUnknown` | May have reached worker; suppress success, cancel/quiesce or quarantine |
| Enrollment/Removal | `SendMutatingCommand` | `MutationDispatchAccepted` | Command accepted; **not** hardware completion |
| Enrollment/Removal | `SendMutatingCommand` | `MutationDispatchNotSent` | After durable arm, remains uncertain by baseline; reconcile; no blind restart/retry |
| Enrollment/Removal | `SendMutatingCommand` | `MutationDispatchUnknown` | Reconcile/quarantine, no retry |

The `*Accepted` signal is optional to determine eventual hardware outcome; a terminal provider observation is still independently correlated and checked. `MutationDispatchNotSent` MUST NOT be treated as a recovery proof after `DispatchArmed`: failure of one send attempt does not prove the overall TA state. If a trusted, independently verifiable no-effect result exists it enters the **reconciliation verifier** as `PreStateVerified`, not via the send receipt. A mutation effect cannot exist before durable arm. An untrusted provider cannot assert `NotSent` as a substitute for the effect runner.

The actor may retain one outer mailbox `OperationMsg`, but messages are transformed into these typed domain inputs only after correlation to `EffectId`, `CommandId`, `OperationId`, `WorkerInstanceId`, epoch and **the `OperationBody` variant**. Any cross-machine acknowledgement is `InvalidInputForOperation`, state unchanged; count/trace a protocol violation. Do not map a verification send receipt to a mutation operation or vice versa.

## C-06: replacement reconciliation is three-way, not boolean

For `EnrollRequest.replace = Some(old)` the independent verifier compares the observed complete affected enrollment scope against **both** frozen pre-state and expected desired post-state. They are different registrations. It returns:

- `DesiredStateConfirmed(VerifiedState)` only on a verified match to the expected new registration and disappearance/replacement of the original as required by the operation.
- `PreStateConfirmed(VerifiedState)` only if the entire original state is proven unchanged, including the original template/registration.
- `Indeterminate(ReconciliationFailure)` for a partial mutation, unknown observation, mismatched identity/generation, or state matching neither endpoint (including “old removed, new not present”).

Both affirmative verdicts still enter `Reconciling → Committing → Finalizing → Terminal` per 27. An unmatched observation is **never** `NoChange` and never causes another hardware mutation automatically.

## Required conformance

- `E-08`: `Verification` state cannot carry a mutation context by construction.
- `E-09`: `Enrollment`/`Removal` cannot be created without their mutation context/plan.
- `E-10`: Cross-machine send receipt rejected with unchanged state/effects/transition index.
- `E-11`: `Enroll(replace)` observed partial state (old absent/new absent) → Indeterminate, no metadata success.
- `E-12`: Same original enrollment unchanged → PreStateVerified → metadata NoChange commit.
- `E-13`: New expected enrollment fully confirmed → DesiredStateVerified → Registry CAS commit.
- `E-14`: Armed mutation `MutationDispatchNotSent` / `MutationDispatchUnknown` does not retry or become NoChange without independent proof.

**No compiler/runtime test is claimed merely because these documents exist.** If the implementation has a different actual input/receipt representation, issue `ARCHITECTURE_BLOCKER` and request an explicit ADR before changing public behaviour.

### v0.6.6 enrollment create/replace projection (normative override)

The internal machine remains `OperationBody::Enrollment`; `EnrollRequest.replace` remains the original choice, now frozen into `MutationPlanRecord.transition` and `old_target`. The runtime's typed dispatch projection MUST distinguish `EnrollmentCommandChange::Create` and `::Replace { old: ValidatedEnrollmentBinding }`; there are no two-empty-optionals semantics. See [34](34-v066-enrollment-command-projection.md). This does not add a new Engine kind or permit a post-arm retry.
