use super::*;
use crate::mutation_plan::{BindingSnapshot, Scope, Transition, sha256};
use carosign_core::enrollment::EnrollmentGeneration;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, AuthorityId, SubjectId};

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn authenticator() -> AuthenticatorId {
    AuthenticatorId::from_bytes([3u8; 16]).expect("валид")
}

fn scope() -> Scope {
    Scope {
        subject: subject(),
        authenticator: authenticator(),
    }
}

fn entry(byte: u8, reference: &[u8]) -> RegistryEntry {
    RegistryEntry {
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
            authenticator: authenticator(),
            generation: EnrollmentGeneration::FIRST,
        },
        subject: subject(),
        status: EnrollmentStatus::Active,
        kind: StoredBindingKind::ProviderOpaque,
        provider_ref: reference.to_vec(),
        format_id: "goodix-asp".into(),
        format_version: 1,
    }
}

fn snapshot(entry: &RegistryEntry) -> BindingSnapshot {
    BindingSnapshot {
        enrollment: entry.enrollment,
        subject: entry.subject,
        kind: entry.kind,
        provider_ref_digest: sha256(&entry.provider_ref),
        format_id: entry.format_id.clone(),
        format_version: entry.format_version,
    }
}

#[test]
fn create_carries_only_the_new_identity() {
    // Creation with one existing registration.
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([9u8; 16]).expect("валид")),
        1,
        vec![snapshot(&entry(5, b"x"))],
        scope(),
    )
    .expect("план");
    let projection = build(&plan, scope(), &[entry(5, b"x")]).expect("проекция");
    match projection.change {
        ProjectionChange::Create { planned_new } => {
            assert_eq!(planned_new.id.as_bytes()[0], 9)
        }
        other => panic!("ожидалась новая регистрация, а пришло {other:?}"),
    }
}

#[test]
fn first_enrollment_uses_the_validated_empty_scope() {
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([9; 16]).unwrap()),
        1,
        Vec::new(),
        scope(),
    )
    .unwrap();
    let projection = build(&plan, scope(), &[]).unwrap();
    assert!(
        matches!(projection.change, ProjectionChange::Create { planned_new }
            if planned_new.authenticator == authenticator()
                && planned_new.id == EnrollmentId::from_bytes([9; 16]).unwrap())
    );
    let foreign = Scope {
        subject: subject(),
        authenticator: AuthenticatorId::from_bytes([7; 16]).unwrap(),
    };
    assert_eq!(build(&plan, foreign, &[]), Err(ProjectionError::PlanShape));
    let mut damaged = plan;
    damaged.new_generation = 2;
    assert_eq!(
        build(&damaged, scope(), &[]),
        Err(ProjectionError::PlanShape)
    );
}

#[test]
fn replace_finds_the_exact_old_target() {
    // B14-02: Replace A with B when C is present – A is taken.
    let a = entry(1, b"a");
    let c = entry(2, b"c");
    let plan = MutationPlan::build(
        Transition::EnrollReplace,
        Some(a.enrollment),
        Some(EnrollmentId::from_bytes([9u8; 16]).expect("валид")),
        1,
        vec![snapshot(&a), snapshot(&c)],
        scope(),
    )
    .expect("план");
    let projection = build(&plan, scope(), &[a.clone(), c]).expect("проекция");
    match projection.change {
        ProjectionChange::Replace { planned_new, old } => {
            assert_eq!(planned_new.id.as_bytes()[0], 9);
            assert_eq!(old.enrollment, a.enrollment);
        }
        other => panic!("ожидалась замена, а пришло {other:?}"),
    }
}

#[test]
fn missing_old_fails_before_arm() {
    // B14-03: no old purpose on the registry - denial before authorization.
    let a = entry(1, b"a");
    let plan = MutationPlan::build(
        Transition::EnrollReplace,
        Some(a.enrollment),
        Some(EnrollmentId::from_bytes([9u8; 16]).expect("валид")),
        1,
        vec![snapshot(&a)],
        scope(),
    )
    .expect("план");
    assert!(matches!(
        build(&plan, scope(), &[]),
        Err(ProjectionError::OldMissingOrStale { .. }) | Err(ProjectionError::ScopeMismatch)
    ));
}

#[test]
fn changed_reference_fails_before_arm() {
    // B14-04: the link has changed - refusal, untested bundle is not sent.
    let a = entry(1, b"a");
    let plan = MutationPlan::build(
        Transition::EnrollReplace,
        Some(a.enrollment),
        Some(EnrollmentId::from_bytes([9u8; 16]).expect("валид")),
        1,
        vec![snapshot(&a)],
        scope(),
    )
    .expect("план");
    let mut changed = a;
    changed.provider_ref = b"different".to_vec();
    assert_eq!(
        build(&plan, scope(), &[changed]),
        Err(ProjectionError::ReferenceMismatch)
    );
}

#[test]
fn kind_mismatch_is_refused() {
    let a = entry(1, b"a");
    let plan = MutationPlan::build(
        Transition::EnrollReplace,
        Some(a.enrollment),
        Some(EnrollmentId::from_bytes([9u8; 16]).expect("валид")),
        1,
        vec![snapshot(&a)],
        scope(),
    )
    .expect("план");
    let mut vault = a;
    vault.kind = StoredBindingKind::ScopedVault;
    assert_eq!(
        build(&plan, scope(), &[vault]),
        Err(ProjectionError::KindMismatch)
    );
}

#[test]
fn replacement_rejects_changes_to_other_bindings() {
    let old = entry(1, b"old");
    let other = entry(2, b"other");
    let plan = MutationPlan::build(
        Transition::EnrollReplace,
        Some(old.enrollment),
        Some(EnrollmentId::from_bytes([9; 16]).unwrap()),
        1,
        vec![snapshot(&old), snapshot(&other)],
        scope(),
    )
    .unwrap();
    let mut changed = other.clone();
    changed.provider_ref = b"changed".to_vec();
    assert_eq!(
        build(&plan, scope(), &[old.clone(), changed]),
        Err(ProjectionError::ReferenceMismatch)
    );
    assert_eq!(
        build(&plan, scope(), &[old.clone(), old]),
        Err(ProjectionError::ScopeMismatch)
    );
}

#[test]
fn create_rejects_changed_scope_even_when_count_matches() {
    let existing = entry(5, b"original");
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([9; 16]).unwrap()),
        1,
        vec![snapshot(&existing)],
        scope(),
    )
    .unwrap();
    let mut changed = existing;
    changed.format_version += 1;
    assert_eq!(
        build(&plan, scope(), &[changed]),
        Err(ProjectionError::FormatMismatch)
    );
}

#[test]
fn new_identifier_cannot_reuse_any_existing_registration() {
    let existing = entry(5, b"original");
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(existing.enrollment.id),
        1,
        vec![snapshot(&existing)],
        scope(),
    );
    assert!(plan.is_err());
}
