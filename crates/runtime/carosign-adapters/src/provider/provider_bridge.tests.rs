use super::super::ProviderFailure;

use super::super::{VerificationFrameContext, mutate_plan, verification_frame};
use super::*;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::{AuthenticatorId, EnrollmentId};
use carosign_core::ids::{CommandId, MutationId, WorkerInstanceId};
use carosign_runtime::{BindingKind, EnrollmentBinding, PermittedSet};

fn set(count: u8) -> PermittedSet {
    PermittedSet::new(
        (1..=count)
            .map(|byte| EnrollmentBinding {
                enrollment: EnrollmentRef {
                    id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
                    authenticator: AuthenticatorId::from_bytes([9u8; 16]).expect("валид"),
                    generation: EnrollmentGeneration::FIRST,
                },
                kind: BindingKind::ProviderOpaque,
                provider_ref: vec![byte; 4],
                format_id: None,
                format_version: 0,
            })
            .collect(),
    )
    .expect("набор")
}

#[path = "fixtures/provider_bridge/counting_channel.rs"]
mod counting_channel;
use counting_channel::CountingChannel;
#[path = "fixtures/provider_bridge/dummy_channel.rs"]
mod dummy_channel;
use dummy_channel::Dummy;

#[test]
fn verification_carries_the_permitted_set() {
    // ⚠️ D-03: The set is always empty and leaves in the frame.
    let plan = CommandPlan::Verify { permitted: set(2) };
    let frame = ProviderBridge::<Dummy>::frame(
        [1u8; 16],
        1,
        OperationId::from_bytes([2u8; 16]).expect("валид"),
        &plan,
    );
    match frame.payload {
        Some(Payload::StartVerification(start)) => {
            assert_eq!(start.permitted_enrollments.len(), 2)
        }
        other => panic!("ожидалась проверка, а пришло {other:?}"),
    }
}

#[test]
fn complete_verification_frame_round_trips_and_rejects_wrong_device_or_vault() {
    use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{AuthorityId, SubjectId};
    use carosign_core::operation::context::VerificationPurpose;
    use carosign_evidence::recipient::{ChallengeRef, RecipientRef};
    use carosign_provider_sdk::command::Command;
    use carosign_provider_sdk::wire::Channel;
    let mut context = VerificationFrameContext {
        worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
        command: CommandId::from_bytes([2; 16]).unwrap(),
        operation: OperationId::from_bytes([3; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([9; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
        provider_subject: SubjectRef {
            authority: AuthorityId::from_bytes([4; 16]).unwrap(),
            subject: SubjectId::from_bytes([5; 16]).unwrap(),
        },
        uid: 1000,
        purpose: VerificationPurpose::SessionLogin,
        challenge: ChallengeRef::accepted_by_recipient([6; 16]).unwrap(),
        recipient: RecipientRef::new([7; 16]).unwrap(),
    };
    for purpose in [
        VerificationPurpose::SessionLogin,
        VerificationPurpose::SessionUnlock,
        VerificationPurpose::PrivilegedAction,
        VerificationPurpose::FactorSelfCheck,
        VerificationPurpose::KeyOperation,
    ] {
        context.purpose = purpose;
        let frame = verification_frame(&context, &set(2)).unwrap();
        let bytes = Channel::new().prepare(frame).unwrap();
        let decoded = Channel::new().accept(&bytes).unwrap();
        let Command::StartVerification {
            uid,
            permitted_enrollments,
            purpose: decoded_purpose,
            challenge_ref,
            recipient_ref,
            ..
        } = carosign_provider_sdk::command::parse(&decoded).unwrap()
        else {
            panic!("ожидалась проверка")
        };
        assert_eq!(uid, 1000);
        assert_eq!(permitted_enrollments.len(), 2);
        assert_eq!(
            VerificationPurpose::from_wire(decoded_purpose as i32).unwrap(),
            purpose
        );
        assert_eq!(challenge_ref, vec![6; 16]);
        assert_eq!(recipient_ref, vec![7; 16]);
    }
    context.authenticator.id = AuthenticatorId::from_bytes([10; 16]).unwrap();
    assert_eq!(
        verification_frame(&context, &set(2)),
        Err(ProviderFailure::Rejected)
    );
    context.authenticator.id = AuthenticatorId::from_bytes([9; 16]).unwrap();
    let mut binding = set(1).bindings()[0].clone();
    binding.kind = BindingKind::ScopedVault;
    binding.provider_ref.clear();
    let vault = PermittedSet::new(vec![binding]).unwrap();
    assert_eq!(
        verification_frame(&context, &vault),
        Err(ProviderFailure::Rejected)
    );
}

#[test]
fn hat_authorization_travels_as_reference_only() {
    // ⚠️ Document 31: The token does not go through the provider’s wire.
    let plan = mutate_plan(
        MutationId::from_bytes([3u8; 16]).expect("валид"),
        CommandId::from_bytes([4u8; 16]).expect("валид"),
        EnrollmentAuthorization::Reference {
            reference: [5u8; 16],
            challenge_digest: [6u8; 32],
        },
        new_enrollment_change(),
    );
    let frame = ProviderBridge::<Dummy>::frame(
        [1u8; 16],
        1,
        OperationId::from_bytes([2u8; 16]).expect("валид"),
        &plan,
    );
    assert_eq!(frame.command_id, vec![4u8; 16]);
    match frame.payload {
        Some(Payload::StartEnrollment(start)) => {
            assert_eq!(start.enrollment_authorization_ref, vec![5u8; 16]);
            assert_eq!(start.enrollment_challenge_digest, vec![6u8; 32]);
        }
        other => panic!("ожидалась регистрация, а пришло {other:?}"),
    }
}

#[test]
fn none_authorization_sends_no_fields() {
    let plan = mutate_plan(
        MutationId::from_bytes([3u8; 16]).expect("валид"),
        CommandId::from_bytes([4u8; 16]).expect("валид"),
        EnrollmentAuthorization::None,
        new_enrollment_change(),
    );
    let frame = ProviderBridge::<Dummy>::frame(
        [1u8; 16],
        1,
        OperationId::from_bytes([2u8; 16]).expect("валид"),
        &plan,
    );
    match frame.payload {
        Some(Payload::StartEnrollment(start)) => {
            assert!(start.enrollment_authorization_ref.is_empty());
            assert!(start.enrollment_challenge_digest.is_empty());
        }
        other => panic!("ожидалась регистрация, а пришло {other:?}"),
    }
}

#[test]
fn bridge_refuses_mutation_without_ticket_before_channel_send() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let calls = Arc::new(AtomicUsize::new(0));
    let bridge = ProviderBridge::new(CountingChannel(calls.clone()), [1; 16], 1);
    let plan = mutate_plan(
        MutationId::from_bytes([3; 16]).unwrap(),
        CommandId::from_bytes([4; 16]).unwrap(),
        EnrollmentAuthorization::None,
        new_enrollment_change(),
    );
    let result = futures_executor::block_on(
        bridge.dispatch(OperationId::from_bytes([2; 16]).unwrap(), plan),
    );
    assert_eq!(result, DispatchOutcome::NotSent);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn bridge_refuses_verification_without_complete_context() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let calls = Arc::new(AtomicUsize::new(0));
    let bridge = ProviderBridge::new(CountingChannel(calls.clone()), [1; 16], 1);
    let result = futures_executor::block_on(bridge.dispatch(
        OperationId::from_bytes([2; 16]).unwrap(),
        CommandPlan::Verify { permitted: set(1) },
    ));
    assert_eq!(result, DispatchOutcome::NotSent);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

fn new_enrollment_change() -> MutationChange {
    MutationChange::Create {
        planned_new: EnrollmentRef {
            id: EnrollmentId::from_bytes([7u8; 16]).expect("валид"),
            authenticator: AuthenticatorId::from_bytes([9u8; 16]).expect("валид"),
            generation: EnrollmentGeneration::FIRST,
        },
    }
}
