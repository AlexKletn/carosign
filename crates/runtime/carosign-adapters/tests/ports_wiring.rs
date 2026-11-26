//! Ports and Adapters: What Connects and Fails
//!
//! ⚠️ The bundle is checked: the domain requires a port - the adapter gives it; the adapter
//! fails when the performer fails; the assembly does not rise with half the ports.

use carosign_adapters::boot::{BootError, Builder};
use carosign_adapters::clock::StepClock;
use carosign_adapters::evidence::{EvidenceBridge, Unavailable};
use carosign_adapters::provider::ProviderBridge;
use carosign_adapters::resource::{GrantOutcome, ResourceBridge};
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{BootId, LeaseId, OperationId, ResourceId};
use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
use carosign_runtime::TimeSource;
use carosign_runtime::ports::{
    CommandPlan, DispatchOutcome, EvidencePort, EvidenceRefusal, ProviderExecutionPort,
    ResourceDenial, ResourcePort,
};

fn set() -> ResourceSet {
    ResourceSet::new(vec![ResourceClaim {
        id: ResourceId::from_bytes([1u8; 16]).expect("валид"),
        mode: AccessMode::Exclusive,
    }])
    .expect("набор")
}

fn lease() -> LeaseId {
    LeaseId::from_bytes([2u8; 16]).expect("валид")
}

/// Allowed Registrations: No Empty Registration (D-03)
fn permitted_set() -> carosign_runtime::PermittedSet {
    carosign_runtime::PermittedSet::new(vec![carosign_runtime::EnrollmentBinding {
        enrollment: carosign_core::enrollment::EnrollmentRef {
            id: carosign_core::ids::EnrollmentId::from_bytes([5u8; 16]).expect("валид"),
            authenticator: carosign_core::ids::AuthenticatorId::from_bytes([6u8; 16])
                .expect("валид"),
            generation: carosign_core::enrollment::EnrollmentGeneration::FIRST,
        },
        kind: carosign_runtime::BindingKind::ProviderOpaque,
        provider_ref: vec![1, 2, 3],
        format_id: None,
        format_version: 0,
    }])
    .expect("набор")
}

fn operation() -> OperationId {
    OperationId::from_bytes([3u8; 16]).expect("валид")
}

#[path = "fixtures/ports_wiring/channel.rs"]
mod channel;
use channel::Channel;
#[path = "fixtures/ports_wiring/closed_channel.rs"]
mod closed_channel;
use closed_channel::ClosedChannel;
#[path = "fixtures/ports_wiring/broker.rs"]
mod broker;
use broker::Broker;
#[path = "fixtures/ports_wiring/busy_broker.rs"]
mod busy_broker;
use busy_broker::BusyBroker;
#[path = "fixtures/ports_wiring/malformed.rs"]
mod malformed;
use malformed::Malformed;
#[path = "fixtures/ports_wiring/claims_success.rs"]
mod claims_success;
use claims_success::ClaimsSuccess;
#[path = "fixtures/ports_wiring/never.rs"]
mod never;
use never::Never;

#[test]
fn provider_bridge_refuses_incomplete_verification() {
    let bridge = ProviderBridge::new(Channel, [1u8; 16], 1);
    let outcome = futures_executor::block_on(bridge.dispatch(
        operation(),
        CommandPlan::Verify {
            permitted: permitted_set(),
        },
    ));
    assert_eq!(outcome, DispatchOutcome::NotSent);
}

#[test]
fn incomplete_verification_is_refused_before_closed_channel() {
    let bridge = ProviderBridge::new(ClosedChannel, [1u8; 16], 1);
    let outcome = futures_executor::block_on(bridge.dispatch(
        operation(),
        CommandPlan::Verify {
            permitted: permitted_set(),
        },
    ));
    assert_eq!(outcome, DispatchOutcome::NotSent);
}

#[test]
fn legacy_resource_bridge_does_not_grant_without_owner_and_cleanup() {
    let bridge = ResourceBridge::new(Broker);
    assert_eq!(
        futures_executor::block_on(bridge.acquire(&set(), lease())),
        Err(ResourceDenial::Unavailable)
    );

    let busy = ResourceBridge::new(BusyBroker);
    let denied = futures_executor::block_on(busy.acquire(&set(), lease()));
    assert_eq!(denied, Err(ResourceDenial::Unavailable));
}

#[test]
fn resource_bridge_rejects_grant_for_another_lease_or_resource_set() {
    let expected = ResourceId::from_bytes([1; 16]).unwrap();
    let other = ResourceId::from_bytes([2; 16]).unwrap();
    for (issued_lease, resources) in [
        (LeaseId::from_bytes([99; 16]).unwrap(), vec![expected]),
        (lease(), vec![]),
        (lease(), vec![other]),
        (lease(), vec![expected, expected]),
    ] {
        let bridge = ResourceBridge::new(Malformed(GrantOutcome::Granted {
            lease: issued_lease,
            epoch: DeviceEpoch::FIRST,
            resources,
        }));
        assert_eq!(
            futures_executor::block_on(bridge.acquire(&set(), lease())),
            Err(ResourceDenial::Unavailable)
        );
    }
}

fn evidence_fixture(
    id: [u8; 16],
    operation: [u8; 16],
) -> Result<carosign_evidence::material::EvidenceMaterialRef, ()> {
    use carosign_core::operation::context::{BootDeadline, BootInstant};
    let boot = carosign_core::ids::BootId::from_bytes([42; 16]).unwrap();
    let slot = carosign_evidence::store::EvidenceSlot::new(
        carosign_core::ids::EvidenceId::from_bytes(id).map_err(|_| ())?,
        carosign_core::ids::OperationId::from_bytes(operation).map_err(|_| ())?,
        BootDeadline::new(boot, 20),
    );
    slot.hold(zeroize::Zeroizing::new(vec![1]), BootInstant::new(boot, 10))
        .map_err(|_| ())
}

#[test]
fn unavailable_evidence_never_yields_an_assessment() {
    let bridge = EvidenceBridge::new(Unavailable);
    let context = evidence_context();
    let requirements = carosign_evidence::requirements::EvidenceRequirements::local_login();
    let material = evidence_fixture([5; 16], *context.operation.as_bytes()).unwrap();
    let result = futures_executor::block_on(bridge.evaluate(material, &context, &requirements));
    // ⚠️ `AssuranceAssessment` It's not like that. `PartialEq` On purpose - otherwise the
    // estimate could be forged by comparison with the standard.
    match result {
        Err(EvidenceRefusal::Unavailable) => {}
        other => panic!("ожидался отказ проверяющего, а пришло {other:?}"),
    }
}

#[test]
fn owned_evidence_invalidation_waits_for_lease_and_rejects_foreign_store() {
    use carosign_core::ids::{BootId, EvidenceId, OperationId};
    use carosign_core::operation::context::{BootDeadline, BootInstant};
    use carosign_evidence::store::{EvidenceStore, SlotError};
    use std::sync::Arc;

    let boot = BootId::from_bytes([1; 16]).unwrap();
    let now = BootInstant::new(boot, 10);
    let deadline = BootDeadline::new(boot, 20);
    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let id = EvidenceId::from_bytes([2; 16]).unwrap();
    let owner = Arc::new(EvidenceStore::new(1).unwrap());
    let other = Arc::new(EvidenceStore::new(1).unwrap());
    let reference = owner
        .hold(
            id,
            operation,
            deadline,
            zeroize::Zeroizing::new(vec![42]),
            now,
        )
        .unwrap();
    let foreign = other
        .hold(
            id,
            operation,
            deadline,
            zeroize::Zeroizing::new(vec![1]),
            now,
        )
        .unwrap();
    let bridge = EvidenceBridge::with_store(Unavailable, owner.clone());
    assert!(matches!(
        futures_executor::block_on(bridge.invalidate(foreign)),
        Err(EvidenceRefusal::Unavailable)
    ));
    let lease = owner.take(reference.clone(), now).unwrap();
    assert!(matches!(
        futures_executor::block_on(bridge.invalidate(reference.clone())),
        Err(EvidenceRefusal::Unavailable)
    ));
    assert_eq!(lease.still_valid(now), Err(SlotError::Revoked));
    drop(lease);
    assert!(futures_executor::block_on(bridge.invalidate(reference)).is_ok());
}

#[test]
fn accepted_claim_without_enrollment_context_is_not_an_assessment() {
    let bridge = EvidenceBridge::new(ClaimsSuccess);
    let context = evidence_context();
    let requirements = carosign_evidence::requirements::EvidenceRequirements::local_login();
    let material = evidence_fixture([5; 16], *context.operation.as_bytes()).unwrap();
    assert!(matches!(
        futures_executor::block_on(bridge.evaluate(material.clone(), &context, &requirements)),
        Err(EvidenceRefusal::Unverified)
    ));
    assert!(matches!(
        futures_executor::block_on(bridge.invalidate(material)),
        Err(EvidenceRefusal::Unavailable)
    ));
}

#[test]
fn evidence_bridge_rejects_foreign_context_before_checker() {
    let bridge = EvidenceBridge::new(Never);
    let mut context = evidence_context();
    let requirements = carosign_evidence::requirements::EvidenceRequirements::local_login();
    let foreign = evidence_fixture([5; 16], [99; 16]).unwrap();
    assert!(matches!(
        futures_executor::block_on(bridge.evaluate(foreign, &context, &requirements)),
        Err(EvidenceRefusal::Unverified)
    ));
    let material = evidence_fixture([5; 16], *context.operation.as_bytes()).unwrap();
    context.enrollment.id = carosign_core::ids::EnrollmentId::from_bytes([77; 16]).unwrap();
    assert!(matches!(
        futures_executor::block_on(bridge.evaluate(material, &context, &requirements)),
        Err(EvidenceRefusal::Unverified)
    ));
    let mut context = evidence_context();
    let mut bindings = context.permitted.bindings().to_vec();
    let mut foreign_binding = bindings[0].clone();
    foreign_binding.enrollment.id =
        carosign_core::ids::EnrollmentId::from_bytes([78; 16]).unwrap();
    foreign_binding.enrollment.authenticator =
        carosign_core::ids::AuthenticatorId::from_bytes([79; 16]).unwrap();
    bindings.push(foreign_binding);
    context.permitted = carosign_runtime::PermittedSet::new(bindings).unwrap();
    let material = evidence_fixture([5; 16], *context.operation.as_bytes()).unwrap();
    assert!(matches!(
        futures_executor::block_on(bridge.evaluate(material, &context, &requirements)),
        Err(EvidenceRefusal::Unverified)
    ));
}

fn evidence_context() -> carosign_runtime::ports::EvidenceContext {
    let state = carosign_engine::tests_support::initial();
    let permitted = permitted_set();
    let enrollment = permitted.bindings()[0].enrollment;
    carosign_runtime::ports::EvidenceContext {
        operation: operation(),
        subject: state.common.context.subject,
        authenticator: carosign_core::device::AuthenticatorRef {
            id: enrollment.authenticator,
            device_epoch: DeviceEpoch::FIRST,
        },
        enrollment,
        worker: carosign_core::ids::WorkerInstanceId::from_bytes([9; 16]).unwrap(),
        permitted,
        purpose: carosign_core::operation::context::VerificationPurpose::SessionLogin,
        recipient: carosign_evidence::recipient::RecipientRef::new([10; 16]).unwrap(),
        challenge: carosign_evidence::recipient::ChallengeRef::accepted_by_recipient([11; 16])
            .unwrap(),
        deadline: state.common.context.deadline,
    }
}

#[test]
fn boot_refuses_to_start_without_ports() {
    let error = Builder::new().compose().unwrap_err();
    match error {
        BootError::MissingPorts { missing } => {
            // ⚠️ It's the **all* missing, not the first one.
            assert_eq!(missing.len(), 5);
        }
    }
}

#[test]
fn boot_lists_only_what_is_missing() {
    let clock = StepClock::new(BootId::from_bytes([6u8; 16]).expect("валид"));
    let builder = Builder::new().time(Box::new(clock));
    match builder.compose().unwrap_err() {
        BootError::MissingPorts { missing } => {
            assert_eq!(missing.len(), 4);
            assert!(!missing.contains(&"TimeSource"));
        }
    }
}

#[test]
fn clock_is_a_port_not_a_global() {
    let clock = StepClock::new(BootId::from_bytes([7u8; 16]).expect("валид"));
    let before = clock.now();
    clock.advance(1_000);
    assert!(clock.now().instant_ns() > before.instant_ns());
}
