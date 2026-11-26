# 34. v0.6.6 — explicit enrollment command projection and provenance (B-14)

**Status: normative.** This closes the implementation report that `MutationChange::Enrollment` used absent `old` and `desired` for both new enrollment and replacement. It supersedes conflicting command-building and storage-v2 guidance in 06/10/20/29/33. It does **not** move matching, enrollment algorithms, template interpretation or TA commands out of the provider.

## Decision: pass the old binding explicitly, derive it exactly once

`CommandPlan` is a **checked dispatch projection**, not a second mutation authority or another persisted source of truth. Runtime's trusted `EnrollmentCommandBuilder` builds it from immutable, durably prepared `MutationPlanRecord` **plus** an atomic Registry read under the same affected-scope lock. The provider adapter gets a complete `CommandPlan` and serializes `StartEnrollment`; it MUST NOT independently read mutable metadata, select an "old" enrollment, guess the replacement from the first snapshot, or treat empty fields as "new".

The authoritative discrimination is `MutationPlanRecord.transition`, not presence/absence of two nullable values. For internal Rust, the prescribed semantic shape is:

```rust
pub enum MutationChange {
    Enrollment(EnrollmentCommandChange),
    Removal(RemovalCommandChange),
}

pub enum EnrollmentCommandChange {
    Create { planned_new: EnrollmentRef },
    Replace { planned_new: EnrollmentRef, old: ValidatedEnrollmentBinding },
}

pub struct CommandPlan {
    mutation_id: MutationId,
    change: MutationChange,
    // Checked operation, subject, authenticator, worker, epoch, scope and grant
    // context is bound in an opaque trusted context/ticket; no caller-controlled IDs.
    context: AuthorizedCommandContext,
}
```

`CommandPlan` fields and constructors are private to the trusted Runtime builder; its serialized bytes and ID alone grant no authority. `ValidatedEnrollmentBinding` has a private constructor; it contains `EnrollmentRef`, `kind`, provider-opaque reference (when permitted), format metadata, and a non-exportable scoped vault descriptor (when applicable). It is **not** a template, general vault credential or credential/HAT token. The actual Rust struct may be reorganized without changing these alternatives and invariants. `MutationChange::Enrollment` MUST NOT use `(old=None, desired=None)` for either case. `planned_new` is an **identity/generation**, not an unknown future provider reference or a claim of successful enrollment. Enroll-replace always creates a **different** new enrollment ID from the old one.

## Source of truth and authoritative construction

1. `EnrollRequest.replace: Option<EnrollmentRef>` is authenticated and scope-validated. `MutationRepository::prepare` atomically records the original request's intended transition and, for REPLACE, the exact `old_target` **and** immutable full affected-scope snapshots, a generated `new_enrollment_id`/generation, Registry scope lock, and mutation journal. `MutationRecord.target_enrollment` is a redundant checked mirror for REPLACE/REMOVE; it MUST equal `plan.old_target`. NEW requires neither to exist. Invalid, duplicate, stale, or cross-subject old references fail **before PREPARED**.
2. At the last authorized pre-dispatch preflight, Runtime acquires a **consistent Registry snapshot protected by the same scope lock** and finds the **exact `old_target`**, not merely any `pre_bindings` entry. Snapshot may include other unaffected enrollments. It checks current subject/authenticator, ID, generation, full scope membership/count, `StoredBindingKind`, format metadata and SHA-256 of the actual opaque reference against **all** persisted `BindingSnapshot` entries. If the old record is missing/changed, fail closed before arm; do not dispatch. An invalid/unsupported vault binding is a blocker, never a synthetic provider-opaque binding.
3. Runtime creates exactly one `EnrollmentCommandChange::Create` or `::Replace` from this checked result and keeps it tied to operation/mutation/worker/epoch. This volatile projection does not modify the immutable plan or authorize anything on its own. The original full pre-state stays available for three-way reconciliation.
4. Before the first potentially mutating provider/broker call, revalidate manage grant, HAT/challenge if required, device/worker epoch, scope lock, and durable `DISPATCH_ARMED`; only a valid opaque `ArmedMutationTicket` permits sending the single command. For HAT challenge preparation that is proven non-mutating, the existing v0.6.5 pre-arm rules apply unchanged. After arm, a mismatch, loss, or ambiguous dispatch means `NeedsReconciliation`, not auto-rebuilding and retrying.
5. The serializer for provider protocol minor **3** always sets `StartEnrollment.planned_enrollment` to the **new** `EnrollmentRef` planned in storage. CREATE omits `replace` **and** `replace_binding`. REPLACE supplies both, with exactly the old `EnrollmentRef` and validated `EnrollmentBinding`. `StartEnrollment.mutation_id`, context and old/new IDs must correlate to the durable journal/ticket; worker is not permitted to create platform IDs.
6. On `MUTATION_PROVIDER_COMPLETED` for enrollment, `MutationOutcome.enrollment` MUST echo `planned_enrollment` exactly; a missing/wrong echo is a protocol violation, **not** confirmation or `NoChange`. `provider_opaque_ref` comes from the provider and may be empty only under existing strictly scoped provider-managed rules; the Registry binding is committed only following the existing trusted validation/reconciliation path. A correct echo is **not** proof of a physical mutation. On other statuses any enrollment field, if present, must match planned target but cannot drive a commit. Removal uses its separately scoped binding and never invents a new registration ID.

`EnrollmentCommandBuilder` is a privileged Runtime/repository adapter boundary; it MUST NOT be placed in provider SDK, the pure Engine or the hardware worker. The Engine consumes the typed create/replace distinction and immutable plan semantics, not Registry queries.

## Storage v3 / deterministic canonical digest

`StoredValue.schema_version = 3` for newly written records. New `StoredBindingKind` is required in BOTH `EnrollmentRecord` and `BindingSnapshot`: `PROVIDER_OPAQUE` maps exactly to provider wire kind 1; `SCOPED_VAULT` to 2. Zero/unknown kind is invalid. A raw opaque ref may be empty only under provider-specific scoped semantics; an absent ref does **not** imply a vault. For a vault record, the provider-opaque ref is empty, and provider access is only via the separately authorized scoped vault port. That port remains a STOP/ADR if not implemented.

`MutationPlanRecord.old_target` is required **iff** transition is REPLACE or REMOVE. For NEW it must be absent; `MutationRecord.target_enrollment` mirrors it when present. `BindingSnapshot` must contain the exactly selected old target for REPLACE/REMOVE, together with every other affected registration. `new_enrollment_id` is nonzero and distinct from every old enrollment ID for CREATE/REPLACE; `new_generation>=1`. REMOVE has absent new ID and generation zero.

The v3 scope digest is:

```text
SHA256( UTF8("bio-mutation-plan-v3")
  || scope                          // authority[16] subject[16] authenticator[16]
  || transition_u32BE
  || old_target_or_zeroes           // EnrollmentRef: id[16] authenticator[16] generation_u64BE; 40 zero bytes if absent
  || new_id_or_zeroes[16]
  || new_generation_u64BE
  || snapshot_count_u32BE
  || entries )

entry = enrollment_id[16] || authenticator_id[16] || generation_u64BE
     || authority_id[16] || subject_id[16]
     || binding_kind_u32BE
     || provider_ref_digest[32]
     || format_id_len_u16BE || format_id_utf8 || format_version_u32BE
```

Entries are canonically sorted bytewise by `(enrollment_id,generation)`; duplicate or noncanonical order is rejected, not normalized on decode. Counts <=256 and record <=64KiB. Digest is a *consistency checksum* only; it does not authenticate storage or a hardware observation. Registry writes of an enrollment include its `binding_kind` and actual scoped reference where appropriate; no template ever enters the plan. Verification and removal binding resolution follow the same typed custody invariants.

Version 2 has insufficient custody/old-target evidence. Newly prepared records MUST be v3. Unfinished/armed v1 or v2 records are quarantined (not replayed or auto-migrated); completed records can be migrated only after audited verification of binding-kind provenance and terminal outcome. A legacy record's kind may **not** be guessed from an empty ref or device capability alone.

## Negative acceptance scenarios

| Test | Expected |
|---|---|
| B14-01 CREATE plan; no old target, empty pre-state | `Create(planned_new)`; provider `replace` fields absent, `planned_enrollment` present |
| B14-02 REPLACE old=A, planned new=B (B != A); pre-state contains A plus C | `Replace(planned_new=B,old=A)` exactly; C unaffected |
| B14-03 REPLACE old=A but Registry no longer has A / generation changed | Fail before arm; no command constructed |
| B14-04 REPLACE old=A with `provider_ref_digest` or format/kind mismatch | Fail before arm; no unverified binding sent |
| B14-05 REPLACE old=A, provider completed but echoes A or omits enrollment | Protocol fault; no metadata success |
| B14-06 REPLACE observed old absent and new absent | `Indeterminate`, retain quarantine; not `NoChange` |
| B14-07 Loss/timeout after arm before worker result | `NeedsReconciliation`; no resend/reconstruction for automatic retry |
| B14-08 SCOPED_VAULT record with no approved vault port | Unavailable/STOP before arm; no fallback to unscoped provider ref |
| B14-09 v2 armed plan without kind or old target | Quarantine; no synthesized v3 or unsafe re-dispatch |
| B14-10 Provider minor 2 offered, minor 3 not negotiated | Refuse live enrollment/removal/verification under current baseline |
| B14-11 CREATE under a scope with other enrollments | Full pre-state frozen; new ID distinct and old bindings unchanged |
| B14-12 Duplicate/unsorted snapshots or missing `old_target` | Reject persisted plan as corrupt; never infer new/replace from nullable command fields |

**Authority note:** the provider still owns enrollment procedures, sensor/TEE work and matching. This change closes only logical command formation, binding custody and identity correlation; it cannot supply a trusted independent hardware witness. Hardware evidence gates in 30–33 remain open.
