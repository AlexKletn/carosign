# 03. `carosign-core`: exact domain contract v1

**Status:** Rust API baseline for implementation; wire types are separate. All IDs are newtypes, not `type X = Uuid`; default constructor is private. `#![forbid(unsafe_code)]`.

## IDs and canonical encoding

Every identity with name `*Id` below is a 16-byte nonzero opaque ID with canonical byte order, no whitespace/JSON in core. `from_bytes([u8;16]) -> Result<Self, InvalidId>`, `as_bytes(&self) -> &[u8;16]`; zero ID rejected. Creation happens in explicit injected `IdSource` (`OperationId`, `MutationId`, `WorkerInstanceId`, `LeaseId` created by their rightful owners). No `Uuid::new_v4` inside core/engine. `DeviceEpoch(u64)` and `EnrollmentGeneration(u64)` start at **1**, increment via checked addition, overflow is hard error; zero is invalid. `EventSequence` starts at 1 for first published event.

**Names:** `AuthorityId`, `SubjectId`, `ProviderId`, `PackageId`, `ProfileId`, `DeviceId`, `AuthenticatorId`, `EnrollmentId`, `OperationId`, `MutationId`, `CommandId`, `WorkerInstanceId`, `LeaseId`, `GrantId`, `BootId`, `EvidenceId`, `EffectId`. `ProviderId` and `PackageId` in metadata also retain a human-readable package name; **do not** use the string name as hardware identity.

## Core types (normative shape)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubjectRef {
    pub authority: AuthorityId,
    pub subject: SubjectId,
}
pub struct AuthenticatorRef {
    pub id: AuthenticatorId,
    pub device_epoch: DeviceEpoch,
}
pub struct EnrollmentRef {
    pub id: EnrollmentId,
    pub authenticator: AuthenticatorId,
    pub generation: EnrollmentGeneration,
}
pub struct OwnerRef {
    pub authority: AuthorityId,
    pub binding: GrantId,
}
pub struct OperationContext {
    pub operation_id: OperationId,
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorRef,
    pub owner: OwnerRef,
    pub purpose: VerificationPurpose,
    pub deadline: BootDeadline,
}

pub enum VerificationPurpose {
    SessionLogin,
    SessionUnlock,
    PrivilegedAction,
    FactorSelfCheck,
    KeyOperation,
}
pub enum Modality { Fingerprint }
pub enum OperationKind { Verify, Enroll, Remove }
```

`BootDeadline` lives **only** while the machine is running and contains an injected monotonic instant plus `BootId`. It is not persisted as a reusable deadline across reboot. `Purpose` is specified by trusted system authority, not accepted unchanged from untrusted caller. `SubjectRef` is opaque to provider; Linux UID, Veles profile ID and display names live in platform mapping only.

## Separate request, outcome and authority result

```rust
pub struct VerifyRequest {
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorId,
    pub purpose: VerificationPurpose,
}
pub struct EnrollRequest {
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorId,
    pub target: EnrollmentLabel,
    pub replace: Option<EnrollmentRef>,
}
pub struct RemoveRequest {
    pub subject: SubjectRef,
    pub enrollment: EnrollmentRef,
}

pub enum VerificationOutcome {
    Matched(EvidenceId),     // still provider-origin, unaccepted
    NotMatched,
    NoUsableSample,
    Cancelled,
    Failed(VerificationFailure),
}
pub enum MutationOutcome {
    Committed(EnrollmentRef),
    NoChange,
    NeedsReconciliation(MutationId),
    Failed(MutationFailure),
}
```

`EnrollmentLabel` is an optional, bounded client-facing name (at most 64 UTF-8 bytes), never the persistent ID. `replace` requires CAS against the existing generation, not implicit overwrite by `(user, finger)` string. `Identify` deliberately excluded from baseline v1; requires a separately versioned feature/ADR because it changes identity disclosure and authorization.

## Grant and evidence

```rust
pub struct ScopedGrantRef { id: GrantId, issuer: AuthorityId }
pub struct EvidenceRef { id: EvidenceId, operation: OperationId }
pub struct EvidenceClaims { /* untrusted provider claims, never AssuranceAssessment */ }
pub struct AssuranceAssessment { /* verified by trusted assessor, scoped */ }
```

`ScopedGrantRef` is not constructible from arbitrary public bytes, only trusted `PolicyAuthority` port can produce a validated object with lifetime and revocation watcher. `EvidenceRef` is an opaque reference to material retained by the restricted evidence router; public UI never receives raw evidence. `AssuranceAssessment` is produced by trusted verifier/assessor, never by worker.

## Capability semantics

Feature capabilities describe *what functions can be requested*. Minimum: modality, verification mode (`ClaimedSubject`), enrollment and removal flags, template custody (`PlatformManaged | ProviderManaged`), safe query methods, cancellation behavior, interaction mode (`Touch | Swipe | UnderDisplayOptical`), proof format identifiers. Declared features are **not** proof of trust. Unsupported/unknown features → no call.

## Identity and generation invariants

1. `DeviceLocator` is short-lived, never used as DB primary key.
2. `SubjectRef` equality and hashing are the ordered pair `(AuthorityId, SubjectId)`, **never SubjectId alone**. Canonical encoding is exactly `authority[16] || subject[16]` (32 bytes); neither field may be zero. Registry lookup and mutation indexed by `SubjectRef + AuthenticatorId` (48-byte prefix); `EnrollmentRef` additionally pins generation. Different AuthorityId with the same SubjectId are unequal, have distinct scope locks and distinct `enrollment_index` prefixes.
3. Epoch increases when device/worker binding is invalidated; no proof from previous epoch is accepted in a newer operation unless a trusted adapter explicitly re-establishes compatible binding (not v1 automatic).
4. `match` result cannot be implicitly converted to `AcceptedBiometricFactor` or auth success.
5. Corrupt/unknown enum values are **errors**, not defaults.
6. Opaque provider registration ref: max 4096 bytes with a format ID and profile version; never assume it is a biometric template.

## Module ownership

- `ids.rs`: typed IDs, canonical byte validation, generator **trait only**.
- `identity.rs`: Subject/Owner refs; no Unix calls.
- `device.rs`: Device/Authenticator and epochs; no udev.
- `enrollment.rs`: immutable refs/metadata and checked generation.
- `operation/`: values/outcomes, no effect execution.
- `assurance.rs`: properties/assessment refs, not verifier implementation.
- `authorization.rs`: grant shape, no policy decisions.
- `limits.rs`: domain bounds, not runtime mutable config.

### Canonical subject comparison (v0.6.1 clarification)

`SubjectRef::eq` compares **both** newtypes; its `Hash` implementation hashes the same two newtypes in the same order. Never implement a custom `Hash` or `PartialEq` against `subject` alone. The domain identity comparator is **not** Unicode/string-based. Endian-free 16-byte identifiers have no textual normalization. `IdentityAuthority` is the only source of trusted subject bindings; untrusted wire bytes may be decoded syntactically but never used as an authorized subject without resolution. Test C-02 covers equality, hash-map separation and database prefix isolation. See [25 clarifications](25-v061-conflict-resolutions.md).

### Verification dispatch failures and mutation reconciliation v0.6.2

```rust
pub enum VerificationFailure {
    DispatchNotSent(DispatchFailure),
    DispatchUnknown(DispatchFailure),
    ProviderUnavailable,
    ProviderFault,
    DeadlineExceeded,
    // Other pre-existing typed failure variants are not replaced.
}
pub enum ReconciliationVerdict {
    DesiredStateConfirmed(VerifiedState),
    PreStateConfirmed(VerifiedState),
    Indeterminate(ReconciliationFailure),
}
```

`VerifiedState` is a *trusted internal* result carrying source/provenance, authenticator identity, target/baseline enrollment generation, and checked state. It cannot be directly constructed from a worker's bool or client message. `DesiredStateConfirmed` means the specific postcondition of the **operation kind** (Enroll: desired new binding; Remove: target absent) is proven. `PreStateConfirmed` means the full, original precondition is proven unchanged (including existing binding in Enroll-replace). Any mismatch/unknown is `Indeterminate`. Definitions and transition semantics: [27](27-v062-engine-conflict-resolutions.md).

### v0.6.3: `OperationKind` is a derived view, not a second mutable state field

`OperationKind { Verify, Enroll, Remove }` remains the public Core vocabulary. `carosign-engine::OperationState` owns the single tagged `OperationBody` defined in [28](28-v063-operation-discriminant-and-send.md). `kind()` returns the corresponding Core enum by matching its variant; no independent `kind` field and no `mutation.is_some()` dispatch. `EnrollRequest.replace` specifies replacement within `Enroll` without changing the machine kind.

### v0.6.4 — provider-owned match boundary

`VerifyRequest` and `OperationContext` describe authorization and target only; no capture image, template decoder, scoring policy or matcher trait is permitted in Core/Engine. Runtime resolves the scoped authorized `EnrollmentRef` set; only the worker interprets provider refs and executes matching. Core's `Matched(EvidenceId)` points to an internal evidence handle, including a ProviderAssertion synthesized by trusted Runtime when a matched worker has no independent token. Full normative contract: [29](29-v064-provider-owned-matching.md).
