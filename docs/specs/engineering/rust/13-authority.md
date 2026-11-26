# 13. Authority, identities and revocable grants

## Ownership

Policy Authority belongs to host OS. Runtime validates rights but does not decide broad login policy. Linux adapter can use polkit/system rules + re-auth; Veles uses scoped grants and trusted UI flows. `SubjectRef` is stable in owner-specific authority namespace.

## Runtime modules

```text
runtime/src/authz/
  mod.rs
  caller_binding.rs          # authenticated source, NOT wire user name
  subject_resolver.rs         # external identity mapping
  grant_request.rs            # request authorization
  grant_validator.rs          # expiry, scope, epoch
  revocation.rs               # watch owner/authority death
  revalidate.rs               # critical-point checks
  operation_scope.rs          # verify/enroll/remove etc
```

## Ports

`IdentityAuthority::resolve(caller, selector) -> SubjectRef`; `PolicyAuthority::authorize(caller, subject, operation, purpose) -> ScopedGrant`; `PolicyAuthority::validate(grant, snapshot) -> GrantStatus`; `PolicyAuthority::watch(grant) -> RevocationHandle`.

`ScopedGrant` is a *trusted in-process handle*, never constructed by parsing client supplied ID. It binds issuing authority, caller, subject, target authenticator or resource scope, purpose, operation type, expiry and revocation generation. A proxying service needs an explicit delegated request proof.

## Checkpoints

1. Before operation created/queued.
2. Before acquiring privileged device lease, if permissions may have changed.
3. Before dispatch of mutating command (last safe point).
4. Before releasing match/evidence outside trusted boundary.

Revocation after dispatch cannot undo a TA mutation; mark operation uncertain or reconcile when appropriate. Owner connection death cancels future work and stops delivering verification success; observer disconnect only removes observation.

## Enrollment policy

For management (add/remove/replace) matching UID is insufficient; require independent authority approval and re-auth for privileged changes according to OS policy. No provider supplies this approval or grants itself management rights.

## Races

Grant validation is an observation, not an atomic lock over hardware time. If strict revoke-before-mutation is required, the authority owner needs a gate/lease coordinated with dispatch, or semantics must explicitly account for in-flight mutation. No check-then-use illusion.
