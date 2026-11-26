use super::*;

#[test]
fn subject_uid_is_required_for_every_subject_command_only() {
    use provider::provider_frame::Payload as P;
    for payload in [
        P::PrepareEnrollmentChallenge(provider::PrepareEnrollmentChallenge::default()),
        P::StartVerification(provider::StartVerification::default()),
        P::StartEnrollment(provider::StartEnrollment::default()),
        P::StartRemoval(provider::StartRemoval::default()),
    ] {
        let mut frame = provider::ProviderFrame {
            major: limits::MAJOR,
            minor: limits::MINOR_BASELINE,
            payload: Some(payload),
            ..Default::default()
        };
        assert_eq!(parse(&frame), Err(CommandError::Missing { field: "uid" }));
        frame.uid = Some(1000);
        frame.minor = limits::MINOR_BASELINE - 1;
        assert!(matches!(
            parse(&frame),
            Err(CommandError::Wire(WireError::UnsupportedVersion { .. }))
        ));
    }
    let frame = provider::ProviderFrame {
        uid: Some(0),
        payload: Some(P::Shutdown(provider::Shutdown {})),
        ..Default::default()
    };
    assert!(matches!(
        parse(&frame),
        Err(CommandError::Wire(WireError::UnexpectedPayload { .. }))
    ));
}

#[test]
fn challenge_preparation_preserves_platform_scope_and_rejects_other_epoch() {
    let mut frame = provider::ProviderFrame {
        major: limits::MAJOR,
        minor: limits::MINOR_MAX,
        device_epoch: 3,
        operation_id: vec![1; 16],
        uid: Some(0),
        payload: Some(
            provider::provider_frame::Payload::PrepareEnrollmentChallenge(
                provider::PrepareEnrollmentChallenge {
                    authenticator: Some(common::AuthenticatorRef {
                        authenticator_id: vec![2; 16],
                        device_epoch: 3,
                    }),
                    subject: Some(common::SubjectRef {
                        authority_id: vec![3; 16],
                        subject_id: vec![4; 16],
                    }),
                    mutation_id: vec![5; 16],
                },
            ),
        ),
        ..Default::default()
    };
    let parsed = parse(&frame).unwrap();
    assert!(
        matches!(&parsed, Command::PrepareEnrollmentChallenge { uid: 0, mutation, .. } if mutation == &vec![5; 16])
    );
    frame.uid = None;
    assert_eq!(parse(&frame), Err(CommandError::Missing { field: "uid" }));
    frame.uid = Some(1000);
    frame.minor = limits::MINOR_BASELINE - 1;
    assert!(matches!(
        parse(&frame),
        Err(CommandError::Wire(WireError::UnsupportedVersion { .. }))
    ));
    frame.minor = limits::MINOR_MAX;
    assert_eq!(parsed.operation(), Some(&[1; 16][..]));
    assert!(!parsed.is_mutating());
    frame.device_epoch = 4;
    assert!(parse(&frame).is_err());
    frame.device_epoch = 3;
    if let Some(provider::provider_frame::Payload::PrepareEnrollmentChallenge(ref mut value)) =
        frame.payload
    {
        value.mutation_id.clear();
    }
    assert!(parse(&frame).is_err());
}

#[test]
fn cancellation_target_must_match_envelope_operation() {
    let mut frame = provider::ProviderFrame {
        operation_id: vec![1; 16],
        payload: Some(provider::provider_frame::Payload::RequestCancel(
            provider::RequestCancel {
                target_operation_id: vec![2; 16],
            },
        )),
        ..Default::default()
    };
    assert!(matches!(
        parse(&frame),
        Err(CommandError::Wire(WireError::UnexpectedPayload { .. }))
    ));
    if let Some(provider::provider_frame::Payload::RequestCancel(cancel)) = &mut frame.payload {
        cancel.target_operation_id = vec![1; 16];
    }
    assert!(matches!(parse(&frame), Ok(Command::RequestCancel { .. })));
    frame.operation_id.clear();
    assert!(parse(&frame).is_err());
}

#[test]
fn debug_does_not_disclose_recovery_reference() {
    let command = Command::QueryMutation {
        operation: vec![1; 16],
        authenticator: common::AuthenticatorRef {
            authenticator_id: vec![2; 16],
            device_epoch: 1,
        },
        mutation: vec![3; 16],
        recovery_ref: b"private-reference".to_vec(),
    };
    assert_eq!(format!("{command:?}"), "QueryMutation(..)");
}

#[test]
fn removal_requires_matching_scoped_binding() {
    let target = common::EnrollmentRef {
        enrollment_id: vec![1; 16],
        authenticator_id: vec![2; 16],
        generation: 1,
    };
    let binding = provider::EnrollmentBinding {
        enrollment: Some(target.clone()),
        kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
        provider_opaque_ref: vec![8],
        ..Default::default()
    };
    let start = provider::StartRemoval {
        authenticator: Some(common::AuthenticatorRef {
            authenticator_id: vec![2; 16],
            device_epoch: 1,
        }),
        subject: Some(common::SubjectRef {
            authority_id: vec![4; 16],
            subject_id: vec![5; 16],
        }),
        mutation_id: vec![6; 16],
        enrollment: Some(target),
        enrollment_binding: Some(binding),
        provider_opaque_ref: vec![8],
    };
    let frame = |value| provider::ProviderFrame {
        major: limits::MAJOR,
        minor: limits::MINOR_MAX,
        uid: Some(1000),
        operation_id: vec![7; 16],
        device_epoch: 1,
        payload: Some(provider::provider_frame::Payload::StartRemoval(value)),
        ..Default::default()
    };
    assert!(matches!(
        parse(&frame(start.clone())),
        Ok(Command::StartRemoval { binding: actual, .. }) if actual == start.enrollment_binding.clone().unwrap()
    ));
    let mut stale = frame(start.clone());
    stale.device_epoch = 2;
    assert!(parse(&stale).is_err());
    let mut obsolete = frame(start.clone());
    obsolete.minor = limits::MINOR_BASELINE - 1;
    assert!(parse(&obsolete).is_err());
    let mut foreign = start.clone();
    foreign.authenticator.as_mut().unwrap().authenticator_id = vec![3; 16];
    assert!(parse(&frame(foreign)).is_err());
    assert!(
        parse(&frame(provider::StartRemoval {
            enrollment_binding: None,
            ..start.clone()
        }))
        .is_err()
    );
    assert!(
        parse(&frame(provider::StartRemoval {
            provider_opaque_ref: vec![9],
            ..start.clone()
        }))
        .is_err()
    );
    let mut mismatched = start.clone();
    mismatched
        .enrollment_binding
        .as_mut()
        .unwrap()
        .enrollment
        .as_mut()
        .unwrap()
        .generation = 2;
    assert!(parse(&frame(mismatched)).is_err());
    let mut vault = start;
    vault.enrollment_binding.as_mut().unwrap().kind =
        provider::EnrollmentBindingKind::EnrollmentBindingScopedVault as i32;
    assert!(parse(&frame(vault.clone())).is_err());
    vault.provider_opaque_ref.clear();
    vault
        .enrollment_binding
        .as_mut()
        .unwrap()
        .provider_opaque_ref
        .clear();
    assert!(matches!(
        parse(&frame(vault.clone())),
        Ok(Command::StartRemoval { binding, .. }) if Some(&binding) == vault.enrollment_binding.as_ref()
    ));
}

#[test]
fn enrollment_requires_planned_identity_and_consistent_replacement() {
    let old = common::EnrollmentRef {
        enrollment_id: vec![1; 16],
        authenticator_id: vec![2; 16],
        generation: 1,
    };
    let new = common::EnrollmentRef {
        enrollment_id: vec![3; 16],
        ..old.clone()
    };
    let start = provider::StartEnrollment {
        authenticator: Some(common::AuthenticatorRef {
            authenticator_id: vec![2; 16],
            device_epoch: 1,
        }),
        subject: Some(common::SubjectRef {
            authority_id: vec![4; 16],
            subject_id: vec![5; 16],
        }),
        mutation_id: vec![6; 16],
        planned_enrollment: Some(new.clone()),
        ..Default::default()
    };
    let frame = |value| provider::ProviderFrame {
        major: limits::MAJOR,
        minor: limits::MINOR_MAX,
        uid: Some(1000),
        operation_id: vec![7; 16],
        device_epoch: 1,
        payload: Some(provider::provider_frame::Payload::StartEnrollment(value)),
        ..Default::default()
    };
    let mut continuation = frame(start.clone());
    continuation.payload = Some(provider::provider_frame::Payload::ContinueEnrollment(
        provider::ContinueEnrollment {
            enrollment: Some(start.clone()),
        },
    ));
    assert!(parse(&continuation).is_err());
    let mut challenged = start.clone();
    challenged.enrollment_authorization_ref = vec![8; 16];
    challenged.enrollment_challenge_digest = vec![9; 32];
    continuation.payload = Some(provider::provider_frame::Payload::ContinueEnrollment(
        provider::ContinueEnrollment {
            enrollment: Some(challenged),
        },
    ));
    assert!(
        matches!(parse(&continuation), Ok(Command::ContinueEnrollment { enrollment }) if matches!(*enrollment, Command::StartEnrollment { authorization: EnrollmentAuthorization::Challenged { .. }, .. }))
    );
    assert!(matches!(
        parse(&frame(start.clone())),
        Ok(Command::StartEnrollment { change: EnrollmentChange::Create { planned }, authorization: EnrollmentAuthorization::None, .. }) if planned == new
    ));
    let mut stale = frame(start.clone());
    stale.device_epoch = 2;
    assert!(parse(&stale).is_err());
    let mut obsolete = frame(start.clone());
    obsolete.minor = limits::MINOR_BASELINE - 1;
    assert!(parse(&obsolete).is_err());
    let mut foreign = start.clone();
    foreign
        .planned_enrollment
        .as_mut()
        .unwrap()
        .authenticator_id = vec![9; 16];
    assert!(parse(&frame(foreign)).is_err());
    for (reference, digest, valid) in [
        (vec![8; 16], vec![9; 32], true),
        (vec![8; 16], vec![], false),
        (vec![], vec![9; 32], false),
        (vec![0; 16], vec![9; 32], false),
        (vec![8; 16], vec![9; 31], false),
    ] {
        let candidate = provider::StartEnrollment {
            enrollment_authorization_ref: reference,
            enrollment_challenge_digest: digest,
            ..start.clone()
        };
        let result = parse(&frame(candidate));
        assert_eq!(
            matches!(result, Ok(Command::StartEnrollment { authorization: EnrollmentAuthorization::Challenged { reference, challenge_digest }, .. }) if reference == vec![8; 16] && challenge_digest == [9; 32]),
            valid
        );
    }
    assert!(
        parse(&frame(provider::StartEnrollment {
            planned_enrollment: None,
            ..start.clone()
        }))
        .is_err()
    );
    assert!(
        parse(&frame(provider::StartEnrollment {
            replace: Some(old.clone()),
            ..start.clone()
        }))
        .is_err()
    );
    let binding = provider::EnrollmentBinding {
        enrollment: Some(old.clone()),
        kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
        ..Default::default()
    };
    assert!(matches!(
        parse(&frame(provider::StartEnrollment {
            replace: Some(old.clone()),
            replace_binding: Some(binding.clone()),
            ..start.clone()
        })),
        Ok(Command::StartEnrollment { change: EnrollmentChange::Replace { planned, old_binding }, .. }) if planned == new && old_binding == binding
    ));
    let mut foreign_old = old.clone();
    foreign_old.authenticator_id = vec![9; 16];
    let mut foreign_binding = binding.clone();
    foreign_binding.enrollment = Some(foreign_old.clone());
    assert!(
        parse(&frame(provider::StartEnrollment {
            replace: Some(foreign_old),
            replace_binding: Some(foreign_binding),
            ..start.clone()
        }))
        .is_err()
    );
    for invalid in [
        provider::EnrollmentBinding {
            kind: 0,
            ..binding.clone()
        },
        provider::EnrollmentBinding {
            kind: i32::MAX,
            ..binding.clone()
        },
        provider::EnrollmentBinding {
            provider_opaque_ref: vec![1; limits::OPAQUE_REF_MAX + 1],
            ..binding.clone()
        },
        provider::EnrollmentBinding {
            kind: provider::EnrollmentBindingKind::EnrollmentBindingScopedVault as i32,
            provider_opaque_ref: vec![1],
            ..binding.clone()
        },
    ] {
        assert!(
            parse(&frame(provider::StartEnrollment {
                replace: Some(old.clone()),
                replace_binding: Some(invalid),
                ..start.clone()
            }))
            .is_err()
        );
    }
    assert!(
        parse(&frame(provider::StartEnrollment {
            replace: Some(old.clone()),
            replace_binding: Some(binding.clone()),
            planned_enrollment: Some(old),
            ..start
        }))
        .is_err()
    );
}

#[test]
fn present_empty_replace_is_not_treated_as_create() {
    let frame = provider::ProviderFrame {
        major: limits::MAJOR,
        minor: limits::MINOR_MAX,
        uid: Some(0),
        payload: Some(provider::provider_frame::Payload::StartEnrollment(
            provider::StartEnrollment {
                replace: Some(common::EnrollmentRef::default()),
                ..Default::default()
            },
        )),
        ..Default::default()
    };
    assert_eq!(
        parse(&frame),
        Err(CommandError::Wire(WireError::BadId {
            field: "enrollment_id"
        }))
    );
}
