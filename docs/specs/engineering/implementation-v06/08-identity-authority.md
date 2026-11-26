# 08. Identity Authority, policy grants and revocation

## Trust boundary

Public wire `SubjectSelector`/username/UID is **untrusted request data**, not an authorization proof. `CallerContext` only constructed from trusted transport principal (Linux kernel peer identity/cgroup where appropriate; Veles verified Uzel principal) by platform boundary. User-provided `operation_id` or `authority_request_ref` is *not* a bearer credential.

## Three identities

1. **Caller**: process/service requesting an operation, e.g. PAM greeter or Settings app. Its identity is established by platform.
2. **Subject**: account/profile to which enrollments belong (`SubjectRef { authority, subject }`). Platform authority owns mapping, not sensor/worker.
3. **Consumer**: component allowed to accept factor evidence for specific purpose (auth service, PAM adapter, KeyMint recipient). May differ from caller and subject.

These are distinct in all contracts. A privileged caller does not imply right to manage every subject without explicit system policy.

## Trusted authority ports

```rust
pub trait IdentityAuthority: Send + Sync {
    fn resolve_subject<'a>(&'a self, caller: &'a CallerContext,
        selector: &'a SubjectSelector) -> BoxFuture<'a, Result<SubjectRef, AuthorityError>>;
}
pub trait PolicyAuthority: Send + Sync {
    fn authorize<'a>(&'a self, request: AuthorizationRequest<'a>)
        -> BoxFuture<'a, Result<ValidatedGrant, AuthorityError>>;
    fn revalidate<'a>(&'a self, grant: &'a ValidatedGrant,
        boundary: AuthorizationBoundary) -> BoxFuture<'a, Result<(), AuthorityError>>;
    fn watch_revocation(&self, grant: &ValidatedGrant) -> RevocationHandle;
}
```

`BoxFuture` is an internal Rust abstraction, not wire ABI. `ValidatedGrant` constructor private, fields encapsulated; contains issuer, caller binding, subject, action scope, purpose, effective generation, monotonic deadline/boot ID and revocation watcher. Do not derive `Serialize` for trusted grant; only `GrantRef` crosses IPC and is resolved by a trusted authority.

## Grant operation scope

`Verify(subject, authenticator, purpose)`, `Enroll(subject, authenticator, replace_ref?)`, `Remove(subject, enrollment_ref)`, `Observe(operation)`, `Cancel(operation)`, `ManageProvider` (system only). Default deny. Verify may be allowed for a system PAM consumer under trusted auth flow; manage factor **must** require a separate confirmation/policy decision. Never grant `Enroll` on `SO_PEERCRED == current_uid` alone.

## Revalidation boundaries

- **B1** before operation creation and resource reservation.
- **B2** before starting verification or mutating dispatch.
- **B3** after provider matched and before releasing factor result/evidence to consumer.
- **B4** before committing profile-owned enrollment binding at platform adapter (where needed).

If revoked before B2, no new mutating command dispatched. If revoked after dispatch armed, operation follows mutation recovery, **not** rollback assumption. Revocation supersedes successful verification result. `ValidatedGrant` cannot be renewed by provider; only Authority can issue a new scoped grant.

## Linux specifics

For each accepted Unix connection, capture peer credentials at accept; optionally verify immutable process properties for sensitive consumers (executables/cgroup), but do not infer trust from process name or PID string alone. `SO_PEERCRED` is connection-bound and does not prove lifetime privilege invariance; track owner lifecycle and revalidate. `polkit` can implement manage-factor grants, with explicit step-up. Root operations require defined system policy; no blanket `uid==0 → all privileges` in core.

## Veles specifics

Veles adapter converts verified principal/capability from Поводник and auth into `CallerContext`/`ValidatedGrant`, with separate independent revocation watcher and generation checks; Core never imports Uzel. `Явь` supplies trusted interaction where established, not authority for match result. Hardware proof remains governed by trust anchor compatibility; synthetic tests do not elevate actual assurance.

## Failure behavior

Lost authority service, grant validation timeout, unrecognized subject authority, ambiguous identity mapping or changed generation → fail closed. Existing potentially mutated hardware scope persists for reconciliation. Audit only pseudonymous ID and policy decision code; never secret input or raw fingerprint.

### v0.6.5: managing a factor is not credential proof

`Enroll` manage grant grants permission to **request** enrollment, not issuance or forging of Gatekeeper Hardware Authentication Token. HAT-required providers need a second, independently checked `EnrollmentCredentialAuthority` challenge-binding flow with dedicated restricted token references. An enrollment challenge issued by a worker cannot be treated as independently trusted without verifying its active hardware/secure session via broker. `TrustedEnrollmentAuthorizationRef` is not public, not a reusable bearer token and is never reconstructed from user wire. Exact requirements and lifecycle: [31](31-v065-enrollment-authorization.md).
