use super::*;
use carosign_core::enrollment::EnrollmentRef;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::EnrollmentId;

#[test]
fn request_digest_binds_exact_label_and_scope() {
    let first = canonical_request_digest(scope(), Transition::EnrollNew, None, Some("")).unwrap();
    let second =
        canonical_request_digest(scope(), Transition::EnrollNew, None, Some("test")).unwrap();
    assert_ne!(first, second);
    assert!(canonical_request_digest(scope(), Transition::EnrollNew, None, None).is_err());
    assert!(canonical_request_digest(scope(), Transition::Remove, None, Some("test")).is_err());
}
use carosign_core::enrollment::EnrollmentGeneration;
use carosign_core::ids::{AuthenticatorId, AuthorityId, SubjectId};

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn scope() -> Scope {
    Scope {
        subject: subject(),
        authenticator: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
    }
}

fn snapshot(byte: u8) -> BindingSnapshot {
    BindingSnapshot {
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
            authenticator: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
            generation: EnrollmentGeneration::FIRST,
        },
        subject: subject(),
        kind: StoredBindingKind::ProviderOpaque,
        provider_ref_digest: [byte; 32],
        format_id: "goodix-asp".into(),
        format_version: 1,
    }
}

#[test]
fn sha256_matches_known_vectors() {
    // ⚠️ The digest must match any other SHA-256.
    assert_eq!(
        sha256(b""),
        [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55
        ]
    );
    assert_eq!(
        sha256(b"abc"),
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad
        ]
    );
}

#[test]
fn plan_rejects_noncanonical_order_and_digests_canonical_order() {
    assert_eq!(
        MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([9; 16]).unwrap()),
            1,
            vec![snapshot(5), snapshot(4)],
            scope(),
        ),
        Err(PlanError::Unsorted)
    );
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([9u8; 16]).expect("валид")),
        1,
        vec![snapshot(4), snapshot(5)],
        scope(),
    )
    .expect("план");
    assert_eq!(plan.pre_bindings[0].enrollment.id.as_bytes()[0], 4);
    assert!(plan.validate(scope()).is_ok());
}

#[test]
fn plan_rejects_two_generations_of_the_same_registry_identity() {
    let first = snapshot(4);
    let mut second = first.clone();
    second.enrollment.generation = EnrollmentGeneration::new(2).unwrap();
    assert_eq!(
        MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([9; 16]).unwrap()),
            1,
            vec![first, second],
            scope(),
        ),
        Err(PlanError::Unsorted)
    );
}

#[test]
fn digest_tampering_is_refused() {
    let mut plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([9u8; 16]).expect("валид")),
        1,
        vec![snapshot(4)],
        scope(),
    )
    .expect("план");
    plan.pre_bindings[0].format_version = 2;
    assert_eq!(plan.validate(scope()), Err(PlanError::DigestMismatch));
    assert_eq!(plan.to_core(scope()), Err(PlanError::DigestMismatch));
}

#[test]
fn core_plan_preserves_exact_replacement_and_full_scope() {
    use carosign_core::operation::plan::PlanTransition;
    let old = snapshot(4);
    let other = snapshot(5);
    let new = EnrollmentId::from_bytes([9; 16]).unwrap();
    let plan = MutationPlan::build(
        Transition::EnrollReplace,
        Some(old.enrollment),
        Some(new),
        2,
        vec![old.clone(), other.clone()],
        scope(),
    )
    .unwrap();
    let semantic = plan.to_core(scope()).unwrap();
    assert_eq!(semantic.pre_bindings().len(), 2);
    assert_eq!(semantic.pre_bindings()[0].enrollment, old.enrollment);
    assert_eq!(semantic.pre_bindings()[1].enrollment, other.enrollment);
    assert_eq!(semantic.scope_digest(), &plan.scope_digest);
    assert!(
        matches!(semantic.transition(), PlanTransition::Replace { old: target, planned_new } if target == old.enrollment && planned_new.id == new && planned_new.generation.value() == 2)
    );
}

#[test]
fn absent_old_target_requires_all_forty_zero_bytes() {
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([9u8; 16]).unwrap()),
        1,
        vec![],
        scope(),
    )
    .unwrap();
    let bytes = encode_plan(&plan).unwrap();
    assert_eq!(decode_plan(&bytes).unwrap(), plan);
    // After the u32 type of transition: 16 bytes of ID, 16 devices and 8 generations.
    for position in 20..44 {
        let mut corrupted = bytes.clone();
        corrupted[position] = 1;
        assert_eq!(decode_plan(&corrupted), Err(PlanError::OldTargetShape));
    }
}

#[test]
fn remove_has_no_new_id_and_zero_generation() {
    let plan = MutationPlan::build(
        Transition::Remove,
        Some(snapshot(4).enrollment),
        None,
        0,
        vec![snapshot(4)],
        scope(),
    )
    .expect("план");
    assert!(plan.validate(scope()).is_ok());
    // ⚠️ Remove the number - the form does not converge.
    let bad = MutationPlan {
        transition: Transition::Remove,
        new_enrollment: Some(EnrollmentId::from_bytes([1u8; 16]).expect("валид")),
        ..plan
    };
    assert_eq!(bad.validate(scope()), Err(PlanError::TransitionShape));
}

#[test]
fn out_of_scope_snapshot_is_refused() {
    let mut foreign = snapshot(4);
    foreign.enrollment.authenticator = AuthenticatorId::from_bytes([7u8; 16]).expect("валид");
    let plan = MutationPlan::build(
        Transition::Remove,
        Some(foreign.enrollment),
        None,
        0,
        vec![foreign.clone()],
        scope(),
    );
    // The assembly will count the digest, but the inspection of the area will refuse.
    match plan {
        Ok(plan) => assert_eq!(plan.validate(scope()), Err(PlanError::OutOfScope)),
        Err(error) => assert_eq!(error, PlanError::OutOfScope),
    }
}
