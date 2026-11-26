use super::*;
use crate::provider_supervisor::ProviderProfile;
use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::{AuthenticatorId, EnrollmentId, OperationId, ResourceId};
use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
use carosign_runtime::operation::bindings::{BindingKind, EnrollmentBinding};
use carosign_runtime::ports::ResourceOwner;

#[test]
#[ignore = "нужны заранее собранные CAROSIGN_TEST_BROKER и CAROSIGN_TEST_PROVIDER"]
fn real_supervisor_lease_broker_and_mock() {
    let profile = ProviderProfile::fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let mut supervisor = ProviderSupervisor::start(
        &profile,
        carosign_linux::sys::boot_id().unwrap(),
        DeviceEpoch::FIRST,
        Duration::from_secs(5),
    )
    .unwrap();
    let resources = LocalResourceOwner::default();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([8; 16]).unwrap(),
        worker: supervisor.worker_id(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([5; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let set = ResourceSet::new(vec![ResourceClaim {
        id: ResourceId::from_bytes([7; 16]).unwrap(),
        mode: AccessMode::Exclusive,
    }])
    .unwrap();
    let lease = futures_executor::block_on(resources.acquire_for(owner, &set)).unwrap();
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([15; 16]).unwrap(),
        authenticator: owner.authenticator.id,
        generation: EnrollmentGeneration::FIRST,
    };
    let permitted = PermittedSet::new(vec![EnrollmentBinding {
        enrollment,
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![21; 16],
        format_id: None,
        format_version: 0,
    }])
    .unwrap();
    let frame = ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: owner.worker.as_bytes().to_vec(),
        frame_seq: 0,
        command_id: vec![11; 16],
        operation_id: owner.operation.as_bytes().to_vec(),
        device_epoch: 1,
        uid: Some(1000),
        payload: Some(Payload::StartVerification(provider::StartVerification {
            authenticator: Some(common::AuthenticatorRef {
                authenticator_id: vec![5; 16],
                device_epoch: 1,
            }),
            subject: Some(common::SubjectRef {
                authority_id: vec![11; 16],
                subject_id: vec![12; 16],
            }),
            purpose: common::VerificationPurpose::PurposeSessionLogin as i32,
            challenge_ref: vec![13; 16],
            recipient_ref: vec![14; 16],
            permitted_enrollments: vec![provider::EnrollmentBinding {
                enrollment: Some(common::EnrollmentRef {
                    enrollment_id: vec![15; 16],
                    authenticator_id: vec![5; 16],
                    generation: 1,
                }),
                kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
                provider_opaque_ref: vec![21; 16],
                format_id: String::new(),
                format_version: 0,
            }],
        })),
    };
    if std::env::var_os("CAROSIGN_TEST_PREPARE_SCOPE").is_some() {
        let lease_scope = CleanupScope::of(&lease);
        let channel = prepare_tunnel(
            &mut supervisor,
            &resources,
            &lease_scope,
            [2; 16],
            [3; 16],
            Duration::from_secs(5),
        )
        .unwrap();
        let mut exchange =
            VerificationExchange::start_channel(channel, frame, permitted, Duration::from_secs(5))
                .unwrap();
        assert!(matches!(
            exchange.next(Duration::from_secs(5)),
            Ok(VerificationMessage::Accepted)
        ));
        assert!(matches!(
            exchange.next(Duration::from_secs(5)),
            Ok(VerificationMessage::Outcome { .. })
        ));
        drop(exchange);
        supervisor.stop_and_confirm(&resources).unwrap();
        assert_eq!(
            futures_executor::block_on(resources.release(lease)),
            QuiesceAssessment::Quiesced
        );
    } else {
        let outcome = futures_executor::block_on(verify(
            &mut supervisor,
            &resources,
            lease,
            [2; 16],
            [3; 16],
            frame,
            permitted,
            Duration::from_secs(5),
        ));
        assert!(matches!(outcome, Ok(Outcome::Observation(_))));
    }
    // After the confirmed cleaning, the same set is available to the new worker.
    let mut next_owner = owner;
    next_owner.worker = carosign_core::ids::WorkerInstanceId::from_bytes([19; 16]).unwrap();
    assert!(futures_executor::block_on(resources.acquire_for(next_owner, &set)).is_ok());
}
