# 12. `carosign-evidence`: proof routing and assessments

## Modules

```text
src/
  lib.rs
  material.rs             # bounded opaque evidence / software report
  context.rs              # operation, subject, challenge, epochs
  verifier.rs             # EvidenceVerifier port
  evaluator.rs            # requirements vs independently assessed facts
  assessment.rs           # capability provenance, expiry, scope
  compat.rs               # acceptable token recipients/versions
  recipient.rs            # protected consumer / key-operation binding
  retention.rs            # handles, zeroization, timeouts
  cache.rs                # carefully scoped assessments only
  errors.rs
```

## Trust separation

Provider declares functional ability and returns `MatchObservation` + optional material. Trusted runtime validates current operation/context and passes material to `EvidenceVerifier`; verifier only accepts cryptographic provenance when actual recipient/anchor confirms it. `AssuranceAssessment` is produced by independent trusted source, not by provider manifest.

## API sketch

```rust
pub trait EvidenceVerifier: Send + Sync {
    fn verify<'a>(&'a self, context: &'a VerificationContext, evidence: &'a EvidenceMaterial) -> VerificationFuture<'a>;
}
pub struct EvidenceAcceptance { pub context: VerificationContextId, pub accepted_properties: VerifiedProperties, pub scope: AcceptanceScope }
```

`VerificationFuture` is an illustrative executor-neutral boxed future alias. `EvidenceAcceptance` does not itself authorize login or decrypt secrets: external Authentication Authority/secure key consumer decides.

## Evidence cases

- `SoftwareMatchReport`: trusted only within software process/OS model; may support PAM policy but is not a hardware signature.
- `ProviderAssertion`: explicitly labeled as asserted only, cannot satisfy hardware proof requirements.
- `OpaqueHardwareToken`: forwarded by restricted handle to compatible verifier; raw token present ≠ token valid.

## Replay resistance

Context binds operation, authenticator epoch, subject, purpose, time and challenge. Challenge provenance may be controlled by protected recipient rather than runtime RNG. Reject stale generations, expired assessment and wrong recipient. Never convert an old `Matched` into a new session grant.

## Diagnostics

Log type/category and verifier verdict, never raw token, template, matching embeddings or secret payload. Failure to verify hardware source returns `Unverified`, not silent `SoftwareMatch` elevation.
