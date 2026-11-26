use crate::enrollment::EnrollmentRef;
use crate::identity::SubjectRef;
use crate::ids::AuthenticatorId;

use super::*;
use crate::enrollment::EnrollmentGeneration;
use crate::ids::{AuthorityId, EnrollmentId, SubjectId};

#[test]
fn same_enrollment_cannot_appear_with_two_generations() {
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([1; 16]).unwrap(),
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([4; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let first = BindingSnapshot {
        enrollment,
        subject,
        kind: BindingKind::ProviderOpaque,
        provider_ref_digest: [5; 32],
        format_id: "example".into(),
        format_version: 1,
    };
    let mut second = first.clone();
    second.enrollment.generation = EnrollmentGeneration::new(2).unwrap();
    let planned_new = EnrollmentRef {
        id: EnrollmentId::from_bytes([6; 16]).unwrap(),
        ..enrollment
    };
    assert!(matches!(
        MutationPlan::new(
            subject,
            authenticator,
            PlanTransition::Create { planned_new },
            vec![first, second],
            [0; 32]
        ),
        Err(PlanError::NonCanonicalBindings)
    ));
}
