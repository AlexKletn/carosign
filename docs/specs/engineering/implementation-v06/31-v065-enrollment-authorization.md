# 31. v0.6.5 — B-10: challenged enrollment authorization

**Status: normative generic interface** for Provider protocol 1.2. The model intentionally supports both HAT-required and HAT-free hardware without making Android/Gatekeeper universal. The platform **does not implement fingerprint matching**.

## 1. Distinct authorities

- **Manage grant:** policy permits a caller to request enrollment for `SubjectRef`; it does not demonstrate knowledge of the user's credential and is not an HAT.
- **Challenge:** hardware/provider-scoped ephemeral nonce bound to the current trusted session. Goodix/QSEE may supply an Android-compatible challenge; its exact shape and encoding remain provider-specific.
- **Credential authority:** trusted, platform-supplied service asks for any necessary step-up and obtains a correctly signed enrollment authorization from a compatible credential/secure service. It never accepts a worker's unsigned claims as authority.
- **Hardware verifier/sink:** the exact approved target (e.g. enrolled Goodix TA) enforces the hardware token's semantics. Platform does not claim cryptographic HAT validation merely because it forwarded a ref.
- **Provider:** drives device/TA-specific pre-enroll, capture/enroll and post-enroll. It receives a scoped **authorization reference**, not a password or raw HAT on the provider control protocol.

## 2. Mode negotiation and wire

`CapabilityDescriptor.enrollment_authorization` is REQUIRED and nonzero for any worker advertising `PROVIDER_ENROLL`:

- `ENROLL_AUTH_NONE`: no separate challenged HAT step. Existing managed grant and durable mutation fence remain mandatory.
- `ENROLL_AUTH_HAT_CHALLENGE`: provider must support `PrepareEnrollmentChallenge`, `EnrollmentChallengeReady`, and reference-bound `StartEnrollment`; trusted external credential authority and broker/secure recipient are required. No raw token in provider IPC.

A worker requiring HAT but advertising NONE is nonconformant and MUST NOT activate enrollment. A worker with HAT mode but no compatible credential authority/secure sink may still serve **independently supported verification**, but enrollment is unavailable. No fallback to diagnostic/test HAL enrollment or fabricated token.

`PrepareEnrollmentChallenge` and its result are correlated by operation/command/worker instance/device epoch. The returned `(challenge,challenge_ref)` must be backed by trusted broker/secure service's challenge-session record, not accepted solely from untrusted worker bytes. `challenge_ref` has 16 nonzero bytes, challenge 1..64 bytes, `validity_ms` >0 and capped by local policy. Challenge bytes never prove successful registration.

`StartEnrollment` fields `enrollment_authorization_ref` (16B) and `enrollment_challenge_digest` (SHA-256 32B) are **both required exactly** for HAT mode and **both absent** for NONE. Exact prior `replace_binding` rules remain unchanged. Runtime only creates a start command after an approved, unexpired token reference is bound to the same active challenge. Existing mutation scope, epoch and generation fencing are mandatory.

## 3. Trusted enrollment authorization reference

An internal, non-public `EnrollmentAuthorizationRef` indexes a protected, non-serializable authority record:

`{operation_id, mutation_id, subject_ref, authenticator_id, device_epoch, worker_instance_id, challenge_ref, SHA256(challenge), recipient/TA binding, credential-authority ID, grant generation, expiry(boot-bound), single-use status}`.

The authority service checks the credential step-up and token issuance result, caller and scope, compatible challenge representation and sink. The protected token remains within the authorized trust boundary (credential authority + audited broker/sink). It **must not** be persisted in MutationJournal, logged, exposed to public client or handed to a generally untrusted worker. An opaque reference alone is not a bearer capability: on each trusted consumption, broker/sink verifies worker, active lease, operation/mutation/command, grant, epoch, digest, recipient, freshness and unused status. A retained reference from another session/worker can never grant enrollment.

Hardware-specific service may require token bytes in a privileged TA call: the **audited handler** injects them into the precisely typed TA arguments. A worker that can only call a direct unsandboxed HAL accepting raw HAT is not compatible with this trust policy without a separate approved trusted-provider design; do not weaken the general boundary to make it work.

## 4. Ordering and durable mutation fence

1. Resolve subject and approve `Enroll` grant; acquire exclusive scoped lock; durably commit `MUTATION_PREPARED` with immutable MutationPlan (33).
2. Acquire valid hardware lease, revalidate policy; ensure provider mode and compatible credential/broker services.
3. `PrepareEnrollmentChallenge` may run **only if** its provider-specific hardware behavior is audited not to mutate persistent enrollment. Any unproven persistent side effect is a STOP/ADR, not permission to perform it before arming. Provider returns challenge and broker-verified session ref.
4. Credential authority performs trusted step-up and issues restricted `EnrollmentAuthorizationRef`. If denied/expired/revoked before arm, close challenge session, quiesce and durably finalize **NoChange only after proof no persistent mutation was dispatched**.
5. Revalidate grant and reference; durably arm **one** mutating command with OperationId/MutationId/CommandId before sending `StartEnrollment` or any broker handler capable of persistent mutation.
6. `StartEnrollment` carries that auth ref/digest, never the HAT. Broker validates and atomically consumes its permitted use at the hardware side-effect boundary. A failed or ambiguous consumption after arm is `NeedsReconciliation`, not a safe retry.
7. Provider performs enrollment, emits progress and a terminal mutation observation. Post-enroll challenge invalidation is provider/secure-sink responsibility; cleanup failures retain uncertainty. Registry CAS and terminal delivery follow 27/28.

Challenge preparation cannot be silently repeated across worker restart; a new worker/epoch invalidates pending challenge/token reference. After `DispatchArmed`, loss of token, worker, broker or ACK **never** authorizes repeating enrollment. Any late HAT/token after cancel/revoke is discarded. The expiry is based on trusted monotonic/boot-scoped time; the worker's `validity_ms` is at most an input bound, not the authority's clock.

## 5. Terminal and error semantics

- Missing credential authority, invalid `challenge_ref`, unsupported challenge encoding, credential rejection or expiry **before arm**: no `StartEnrollment`; typed pre-dispatch failure (not `NotMatched`). Do not claim `NoChange` until safe cleanup is established.
- Invalid/foreign/expired ref **after arm**, broken TA call, broker crash, lost ACK or provider crash: reconcile/quarantine; no blind resubmission.
- A token accepted by Goodix is **not** proof that template persistence succeeded. `MutationOutcome` and independent reconciliation remain distinct.
- `Enroll(replace)` must compare pre/desired as specified in 28; an absent old+new registration is `Indeterminate`.
- No general HAT parser/credential verifier, password handling or TA command codec in `carosign-core`, Engine or UI.

## 6. Internal ports (implementation binding)

```rust
// Conceptual signatures; types are trusted non-serializable scoped handles.
trait EnrollmentChallengePort {
    fn validate_session(&self, observed: ChallengeObservation,
        op: OperationId, mutation: MutationId) -> Result<ValidatedChallenge, ChallengeError>;
    fn invalidate(&self, session: ValidatedChallenge) -> Result<(), ChallengeError>;
}
trait EnrollmentCredentialAuthority {
    fn authorize(&self, challenge: &ValidatedChallenge,
        grant: &ValidatedGrant, recipient: TrustedEnrollmentRecipient)
        -> BoxFuture<'_, Result<EnrollmentAuthorizationRef, CredentialError>>;
}
trait EnrollmentAuthorizationSink {
    fn consume_for_typed_hardware_call(&self, auth: &EnrollmentAuthorizationRef,
        armed: &ArmedMutationTicket, handler: TrustedHandlerId)
        -> Result<RestrictedTokenMaterial, EnrollmentAuthError>;
}
```

Only the credential authority and audited sink handle any HAT bytes; generic actor/engine messages carry references. The sink's restricted token material remains inside its trusted handler scope and must not be returned to untrusted IPC.

## 7. Mandatory conformance

H-01 valid challenge/session/HAT ref exact scope and hardware sink; H-02 invalid/expired or foreign challenge; H-03 token subject mismatch; H-04 worker/epoch restart; H-05 cancelled before/after arm; H-06 broker denies mutation without durable arm; H-07 no compatible issuer => enrollment unavailable, verify unaffected; H-08 raw token sent in provider/public IPC rejected; H-09 Goodix reaches capture but storage write fails => no Committed; H-10 HAT-free worker starts without token ref only when declared NONE.

**v0.6.6 identity binding:** In HAT mode the challenged `StartEnrollment` MUST additionally carry and correlate `planned_enrollment` to the durable v3 mutation plan; REPLACE contains exact `old_target` binding. The new ref is a platform-assigned identity, not an authorization token. Provider minor >=3 applies; credential authority/HAT rules are otherwise unchanged. [34](34-v066-enrollment-command-projection.md).
