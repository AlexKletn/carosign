use super::*;
use carosign_protocol::carosign::{
    broker_relay::v1::{Claim, Owner},
    common::v1::{AuthenticatorRef, SubjectRef},
};

#[test]
fn challenge_preparation_requires_live_exclusive_scope_and_uid() {
    let context = Context {
        worker: [1; 16],
        epoch: 7,
        broker: [3; 16],
        boot: [4; 16],
    };
    let scope = Scope {
        lease_id: vec![6; 16],
        owner: Some(Owner {
            operation_id: vec![8; 16],
            worker_instance: vec![1; 16],
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        claims: vec![Claim {
            resource_id: vec![7; 16],
            mode: 1,
        }],
    };
    let mut frame = ProviderFrame {
        worker_instance_id: vec![1; 16],
        device_epoch: 7,
        command_id: vec![9; 16],
        operation_id: vec![8; 16],
        uid: Some(0),
        ..Default::default()
    };
    let mut challenge = carosign_protocol::carosign::provider::v1::PrepareEnrollmentChallenge {
        authenticator: Some(AuthenticatorRef {
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        subject: Some(SubjectRef {
            authority_id: vec![10; 16],
            subject_id: vec![11; 16],
        }),
        mutation_id: vec![13; 16],
    };
    assert!(valid_challenge_preparation(
        &context, &scope, &frame, &challenge
    ));
    frame.uid = None;
    assert!(!valid_challenge_preparation(
        &context, &scope, &frame, &challenge
    ));
    frame.uid = Some(1000);
    frame.operation_id[0] ^= 1;
    assert!(!valid_challenge_preparation(
        &context, &scope, &frame, &challenge
    ));
    frame.operation_id[0] ^= 1;
    challenge.authenticator.as_mut().unwrap().device_epoch = 8;
    assert!(!valid_challenge_preparation(
        &context, &scope, &frame, &challenge
    ));
}

#[test]
fn continuation_cannot_change_uid_subject_or_mutation() {
    let subject = SubjectRef {
        authority_id: vec![10; 16],
        subject_id: vec![11; 16],
    };
    let prepared = ChallengePreparation {
        command: vec![14; 16],
        mutation: vec![13; 16],
        subject: subject.clone(),
        uid: 1000,
        accepted: true,
    };
    let mut frame = ProviderFrame {
        command_id: vec![9; 16],
        uid: Some(1000),
        ..Default::default()
    };
    let mut enrollment = carosign_protocol::carosign::provider::v1::StartEnrollment {
        subject: Some(subject),
        mutation_id: vec![13; 16],
        ..Default::default()
    };
    assert!(prepared.matches_continuation(&frame, &enrollment));
    frame.uid = Some(0);
    assert!(!prepared.matches_continuation(&frame, &enrollment));
    frame.uid = Some(1000);
    enrollment.subject.as_mut().unwrap().subject_id[0] ^= 1;
    assert!(!prepared.matches_continuation(&frame, &enrollment));
    enrollment.subject.as_mut().unwrap().subject_id[0] ^= 1;
    enrollment.mutation_id[0] ^= 1;
    assert!(!prepared.matches_continuation(&frame, &enrollment));
}
