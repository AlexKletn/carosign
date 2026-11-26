#[path = "operation_registry_tests_fixtures/authority.rs"]
mod authority;
#[path = "operation_registry_tests_fixtures/before_dispatch.rs"]
mod before_dispatch;
#[path = "operation_registry_tests_fixtures/delivery_authority.rs"]
mod delivery_authority;
#[path = "operation_registry_tests_fixtures/expires_lease.rs"]
mod expires_lease;
#[path = "operation_registry_tests_fixtures/gate_broker.rs"]
mod gate_broker;
#[path = "operation_registry_tests_fixtures/legacy_policy.rs"]
mod legacy_policy;
#[path = "operation_registry_tests_fixtures/live_lease.rs"]
mod live_lease;
#[path = "operation_registry_tests_fixtures/mock_cleanup_broker.rs"]
mod mock_cleanup_broker;
#[path = "operation_registry_tests_fixtures/mock_reply.rs"]
mod mock_reply;
#[path = "operation_registry_tests_fixtures/revoked_during_lease.rs"]
mod revoked_during_lease;
#[path = "operation_registry_tests_fixtures/unavailable_lease.rs"]
mod unavailable_lease;
#[path = "operation_registry_tests_fixtures/unbound_authority.rs"]
mod unbound_authority;
#[path = "operation_registry_tests_fixtures/unreachable_broker.rs"]
mod unreachable_broker;

use crate::operations::{
    OwnerSendError, OwnerSendProgress, PositiveDeliveryOwner, send_reserved_positive,
};

#[test]
fn positive_owner_checks_authority_and_sends_reserved_terminal_once() {
    use super::*;

    use carosign_engine::{
        EffectSpec, OperationPhase, PlannedEffect, TerminalOutcome, TransitionIndex,
    };
    use carosign_protocol::carosign::client::v1::{self as client, client_frame::Payload};
    use carosign_runtime::ports::{AuthorityRefusal, TransactionId};
    use prost::Message;
    use std::os::fd::AsRawFd;

    use delivery_authority::Authority;
    let mut state = carosign_engine::tests_support::initial();
    let id = state.common.context.operation_id;
    let boot = state.common.context.deadline.boot();
    state.common.phase = OperationPhase::Finalizing;
    state.common.terminal = Some(TerminalOutcome::FactorAccepted);
    let effect = PlannedEffect::at(EffectSpec::DeliverOutcome, TransitionIndex::new(1), 0);
    state.common.pending_effects = vec![effect];
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        boot,
        state.common.context.deadline.instant_ns(),
    );
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = carosign_linux::sys::peer_credentials(&pair.platform).unwrap();
    let credentials =
        carosign_linux_ipc::credentials::PeerCredentials::from_kernel(peer.pid, peer.uid, peer.gid)
            .unwrap();
    let mut session =
        carosign_linux_ipc::client::session::ClientSession::new(credentials, [20; 16]);
    let hello = client::ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: vec![1; 16],
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::Hello(client::Hello {
            capture_feedback_v1: false,
            major: 1,
            min_minor: carosign_protocol::limits::MINOR_BASELINE,
            max_minor: carosign_protocol::limits::MINOR_MAX,
        })),
    };
    session.accept(&hello.encode_to_vec()).unwrap();
    let connection = crate::server::Connection {
        session,
        peer,
        fd: pair.platform,
    };
    let caller = CallerContext::from_transport(
        [1; 32],
        peer.uid,
        TransactionId::from_bytes([20; 16]).unwrap(),
    );
    let registry = OperationRegistry::new();
    let mut guard = registry
        .reserve(OperationActor::new(state), grant, &caller)
        .unwrap();
    guard.operation_mut().cleanup_stage = CleanupStage::Released;
    drop(guard);
    let clock = carosign_adapters::clock::StepClock::new(boot);
    let mut delivery = PositiveDeliveryOwner::reserve(
        &registry,
        id,
        caller,
        effect,
        carosign_core::operation::context::BootInstant::new(boot, 0),
    )
    .unwrap();
    assert!(matches!(
        futures_executor::block_on(delivery.try_send_one(&connection, &Authority(false), &clock)),
        Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Authority(
            AuthorityRefusal::Unavailable
        )))
    ));
    let filler = [0u8; 1024];
    loop {
        // SAFETY: The test socket and buffer remain valid throughout the call.
        let sent = unsafe {
            libc::send(
                connection.fd.as_raw_fd(),
                filler.as_ptr().cast(),
                filler.len(),
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        if sent < 0 {
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EAGAIN)
            );
            break;
        }
        assert_eq!(sent as usize, filler.len());
    }
    assert_eq!(
        futures_executor::block_on(delivery.try_send_one(&connection, &Authority(true), &clock)),
        Ok(OwnerSendProgress::WouldBlock)
    );
    let mut drained = [0u8; 2048];
    loop {
        // SAFETY: The test socket and buffer remain valid throughout the call.
        let len = unsafe {
            libc::recv(
                pair.worker.as_raw_fd(),
                drained.as_mut_ptr().cast(),
                drained.len(),
                libc::MSG_DONTWAIT,
            )
        };
        if len < 0 {
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EAGAIN)
            );
            break;
        }
    }
    assert_eq!(
        futures_executor::block_on(delivery.try_send_one(&connection, &Authority(true), &clock)),
        Ok(OwnerSendProgress::Sent)
    );
    assert_eq!(
        futures_executor::block_on(delivery.try_send_one(&connection, &Authority(true), &clock)),
        Err(OwnerSendError::Finished)
    );
    let mut bytes = [0u8; 4096];
    // SAFETY: The descriptor and buffer belong to this test.
    let len = unsafe {
        libc::recv(
            pair.worker.as_raw_fd(),
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            libc::MSG_DONTWAIT,
        )
    };
    assert!(len > 0);
    let frame = client::ClientFrame::decode(&bytes[..len as usize]).unwrap();
    assert_eq!(frame.event_sequence, 1);
    assert!(matches!(frame.payload, Some(Payload::OperationEvent(_))));
}
use super::*;
use crate::provider_verification::VerificationContext;
use crate::resources::{CleanupPortFailure, CleanupScope, QuiescedLease, TrustedCleanupBroker};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

fn fixture_set(
    ids: Vec<carosign_core::ids::ResourceId>,
    mode: carosign_core::resource::AccessMode,
) -> carosign_core::resource::ResourceSet {
    carosign_core::resource::ResourceSet::new(
        ids.into_iter()
            .map(|id| carosign_core::resource::ResourceClaim { id, mode })
            .collect(),
    )
    .unwrap()
}

#[test]
fn mutation_arm_checks_bound_actor_and_exact_predicted_send_effect() {
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentLabel, EnrollmentRef};
    use carosign_core::ids::{
        CommandId, EnrollmentId, LeaseId, MutationId, ResourceId, WorkerInstanceId,
    };
    use carosign_core::operation::context::BootInstant;
    use carosign_core::operation::mutation::EnrollRequest;
    use carosign_core::operation::plan::{MutationPlan, PlanTransition};
    use carosign_engine::{
        EffectSpec, OperationPhase, OperationState, PlannedEffect, TransitionIndex,
    };
    use carosign_runtime::ports::{
        CommandPlan, EnrollmentAuthorization, MutationArmRequest, MutationChange, ResourceLease,
        ResourceOwner, TransactionId,
    };

    let initial = carosign_engine::tests_support::initial();
    let context = initial.common.context;
    let planned_new = EnrollmentRef {
        id: EnrollmentId::from_bytes([41; 16]).unwrap(),
        authenticator: context.authenticator.id,
        generation: EnrollmentGeneration::FIRST,
    };
    let plan = MutationPlan::new(
        context.subject,
        context.authenticator.id,
        PlanTransition::Create { planned_new },
        vec![],
        [0; 32],
    )
    .and_then(|plan| plan.with_label(Some(EnrollmentLabel::new("проба").unwrap())))
    .unwrap();
    let mutation = MutationId::from_bytes([42; 16]).unwrap();
    let mut state = OperationState::enrollment_with_plan(
        context.clone(),
        EnrollRequest {
            subject: context.subject,
            authenticator: context.authenticator.id,
            target: EnrollmentLabel::new("проба").unwrap(),
            replace: None,
        },
        mutation,
        plan.clone(),
    )
    .unwrap();
    let worker = WorkerInstanceId::from_bytes([43; 16]).unwrap();
    let command = CommandId::from_bytes([44; 16]).unwrap();
    let lease_id = LeaseId::from_bytes([45; 16]).unwrap();
    let arm_effect =
        PlannedEffect::at(EffectSpec::PersistDispatchArmed, TransitionIndex::new(4), 0);
    let send_effect =
        PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(6), 0);
    state.common.phase = OperationPhase::Arming;
    state.common.transition_index = TransitionIndex::new(4);
    state.common.pending_effects.push(arm_effect);
    let unbound_state = state.clone();
    let grant = ValidatedGrant::issue_for_port(
        context.owner.binding,
        context.owner.authority,
        state.required_grant_scope(),
        context.deadline.boot(),
        context.deadline.instant_ns(),
    );
    let registry = OperationRegistry::new();
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let revoke_registry = OperationRegistry::new();
    let mut revoke_guard = revoke_registry
        .reserve(OperationActor::new(unbound_state), grant.clone(), &caller)
        .unwrap();
    revoke_guard.operation_mut().lease = Some(ResourceLease::issued_for_fixture(
        lease_id,
        ResourceOwner {
            operation: context.operation_id,
            worker,
            authenticator: context.authenticator,
        },
        fixture_set(
            vec![ResourceId::from_bytes([46; 16]).unwrap()],
            carosign_core::resource::AccessMode::Exclusive,
        ),
    ));
    revoke_guard.operation_mut().lease_matches_request = true;
    drop(revoke_guard);
    let mut guard = registry
        .reserve(OperationActor::new(state), grant, &caller)
        .unwrap();
    guard.operation_mut().lease = Some(ResourceLease::issued_for_fixture(
        lease_id,
        ResourceOwner {
            operation: context.operation_id,
            worker,
            authenticator: context.authenticator,
        },
        fixture_set(
            vec![ResourceId::from_bytes([46; 16]).unwrap()],
            carosign_core::resource::AccessMode::Exclusive,
        ),
    ));
    guard.operation_mut().lease_matches_request = true;
    drop(guard);
    let mut scope = [0; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN];
    scope[..32].copy_from_slice(&context.subject.to_canonical());
    scope[32..].copy_from_slice(context.authenticator.id.as_bytes());
    let request = MutationArmRequest {
        operation: context.operation_id,
        uid: 1000,
        effect: send_effect,
        arm: carosign_persistence::journal::ArmMutation {
            mutation,
            worker,
            epoch: context.authenticator.device_epoch,
            command,
            recovery_ref: None,
        },
        scope,
        label: Some(EnrollmentLabel::new("проба").unwrap()),
        lease: lease_id,
        plan: CommandPlan::Mutate {
            mutation,
            command,
            authorization: EnrollmentAuthorization::None,
            change: MutationChange::Create { planned_new },
        },
        logical_plan: plan,
    };
    let frame = carosign_adapters::provider::mutation_frame(&request).unwrap();
    assert_eq!(frame.uid, Some(1000));
    let now = BootInstant::new(context.deadline.boot(), context.deadline.instant_ns() - 1);
    use before_dispatch::BeforeDispatch;
    use expires_lease::ExpiresLease;
    use legacy_policy::LegacyPolicy;
    use live_lease::LiveLease;
    use revoked_during_lease::RevokedDuringLease;
    use unavailable_lease::UnavailableLease;
    let clock = carosign_adapters::clock::StepClock::new(context.deadline.boot());
    let mut forged = request.clone();
    forged.uid = 0;
    assert!(matches!(
        futures_executor::block_on(registry.bind_mutation_dispatch(
            &forged,
            &clock,
            &BeforeDispatch(false),
            &LiveLease
        )),
        Err(MutationBindingFailure::Operation(
            ObservationFailure::ContextMismatch
        ))
    ));
    assert!(matches!(
        futures_executor::block_on(registry.bind_mutation_dispatch(
            &request,
            &clock,
            &LegacyPolicy,
            &LiveLease,
        )),
        Err(MutationBindingFailure::Authority(
            carosign_runtime::ports::AuthorityRefusal::Unavailable
        ))
    ));
    let revoker = RevokedDuringLease {
        registry: &revoke_registry,
        step: std::sync::Mutex::new(None),
    };
    assert!(
        futures_executor::block_on(revoke_registry.bind_mutation_dispatch(
            &request,
            &clock,
            &BeforeDispatch(false),
            &revoker,
        ))
        .is_err()
    );
    assert!(revoker.step.lock().unwrap().is_some());
    assert!(
        revoke_registry.entries.lock().unwrap()[&context.operation_id]
            .mutation_binding_step
            .is_none()
    );
    assert!(
        revoke_registry
            .checkout(context.operation_id)
            .unwrap()
            .operation_mut()
            .actor
            .state()
            .common
            .worker
            .is_none()
    );
    let busy = registry.checkout(context.operation_id).unwrap();
    assert!(matches!(
        futures_executor::block_on(registry.bind_mutation_dispatch(
            &request,
            &clock,
            &BeforeDispatch(false),
            &LiveLease,
        )),
        Err(MutationBindingFailure::Operation(ObservationFailure::Busy))
    ));
    drop(busy);
    assert!(matches!(
        futures_executor::block_on(registry.bind_mutation_dispatch(
            &request,
            &clock,
            &BeforeDispatch(true),
            &LiveLease,
        )),
        Err(MutationBindingFailure::Authority(
            carosign_runtime::ports::AuthorityRefusal::Revoked
        ))
    ));
    assert!(matches!(
        futures_executor::block_on(registry.bind_mutation_dispatch(
            &request,
            &clock,
            &BeforeDispatch(false),
            &UnavailableLease,
        )),
        Err(MutationBindingFailure::Resource(
            carosign_runtime::ports::ResourceDenial::Unavailable
        ))
    ));
    let expiry_clock = carosign_adapters::clock::StepClock::new(context.deadline.boot());
    expiry_clock.advance(context.deadline.instant_ns().saturating_sub(1));
    assert!(matches!(
        futures_executor::block_on(registry.bind_mutation_dispatch(
            &request,
            &expiry_clock,
            &BeforeDispatch(false),
            &ExpiresLease(&expiry_clock),
        )),
        Err(MutationBindingFailure::Operation(
            ObservationFailure::DeadlineExpired
        ))
    ));
    assert!(
        registry
            .checkout(context.operation_id)
            .unwrap()
            .operation_mut()
            .actor
            .state()
            .common
            .worker
            .is_none()
    );
    assert!(
        futures_executor::block_on(registry.bind_mutation_dispatch(
            &request,
            &clock,
            &BeforeDispatch(false),
            &LiveLease,
        ))
        .is_ok()
    );
    assert!(
        futures_executor::block_on(registry.bind_mutation_dispatch(
            &request,
            &clock,
            &BeforeDispatch(false),
            &LiveLease,
        ))
        .is_err()
    );
    assert!(matches!(
        registry.check_mutation_arm_context(&request, now),
        Err(ObservationFailure::AwaitingActor)
    ));
    let blocked = crate::effect_owner::EffectOwner::default();
    let saved = registry.entries.lock().unwrap()[&context.operation_id]
        .mutation_binding_step
        .clone()
        .unwrap();
    blocked.accept_step(context.operation_id, &saved).unwrap();
    assert!(matches!(
        registry.handoff_mutation_binding(context.operation_id, &blocked),
        Err(crate::effect_owner::EffectOwnerError::Duplicate)
    ));
    let effects = crate::effect_owner::EffectOwner::default();
    registry
        .handoff_mutation_binding(context.operation_id, &effects)
        .unwrap();
    assert!(matches!(
        effects.step(context.operation_id),
        Some(carosign_runtime::operation::actor::Step::Advanced { .. })
    ));
    assert!(
        registry
            .handoff_mutation_binding(context.operation_id, &effects)
            .is_err()
    );
    assert!(registry.check_mutation_arm_context(&request, now).is_ok());
    for foreign in [
        MutationArmRequest {
            label: Some(EnrollmentLabel::new("чужая метка").unwrap()),
            ..request.clone()
        },
        MutationArmRequest {
            effect: PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(6), 1),
            ..request.clone()
        },
        MutationArmRequest {
            effect: PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 0),
            ..request.clone()
        },
        MutationArmRequest {
            lease: LeaseId::from_bytes([99; 16]).unwrap(),
            ..request.clone()
        },
        MutationArmRequest {
            arm: carosign_persistence::journal::ArmMutation {
                worker: WorkerInstanceId::from_bytes([99; 16]).unwrap(),
                ..request.arm.clone()
            },
            ..request.clone()
        },
    ] {
        assert!(matches!(
            registry.check_mutation_arm_context(&foreign, now),
            Err(ObservationFailure::ContextMismatch)
        ));
    }
    assert!(matches!(
        registry.check_mutation_arm_context(
            &request,
            BootInstant::new(context.deadline.boot(), context.deadline.instant_ns())
        ),
        Err(ObservationFailure::DeadlineExpired)
    ));
    let guard = registry.checkout(context.operation_id).unwrap();
    assert!(matches!(
        registry.check_mutation_arm_context(&request, now),
        Err(ObservationFailure::Busy)
    ));
    assert!(matches!(
        registry.revoke_verification(context.operation_id),
        Err(ObservationFailure::Busy)
    ));
    drop(guard);
    assert!(matches!(
        registry.check_mutation_arm_context(&request, now),
        Err(ObservationFailure::AwaitingActor)
    ));
    assert_eq!(
        registry.phase(context.operation_id),
        Some(OperationPhase::Arming)
    );
    let status = registry.live_status(context.operation_id).unwrap();
    assert_eq!(status.subject, context.subject);
    assert_eq!(status.phase, OperationPhase::Arming);
    assert!(status.is_mutation);
    assert_eq!(status.verification_outcome, None);
}

#[test]
fn evidence_effect_waits_for_actual_wipe_and_retains_next_step() {
    use carosign_core::ids::EvidenceId;
    use carosign_core::operation::context::BootInstant;
    use carosign_engine::{DomainInput, EffectSpec, OperationPhase};
    use carosign_runtime::operation::actor::Step;
    use carosign_runtime::{OperationActor, OperationMsg, ports::TransactionId};

    let mut state = carosign_engine::tests_support::initial();
    let operation = state.common.context.operation_id;
    let evidence = EvidenceId::from_bytes([91; 16]).unwrap();
    let now = BootInstant::new(state.common.context.deadline.boot(), 1);
    state.common.phase = OperationPhase::Running;
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let registry = OperationRegistry::new();
    drop(
        registry
            .reserve(OperationActor::new(state), grant, &caller)
            .unwrap(),
    );
    let reference = registry
        .hold_actor_evidence(
            operation,
            evidence,
            zeroize::Zeroizing::new(vec![42; 8]),
            now,
        )
        .unwrap();
    let mut guard = registry.checkout(operation).unwrap();
    guard
        .operation_mut()
        .actor
        .post(OperationMsg::Domain(DomainInput::ProviderMatched(evidence)))
        .unwrap();
    assert!(matches!(
        guard.operation_mut().actor.run_once(),
        Step::Advanced { .. }
    ));
    drop(guard);
    let lease = registry
        .take_actor_evidence(operation, reference.clone(), now)
        .unwrap();
    let guard = registry.checkout(operation).unwrap();
    assert!(matches!(
        registry.revoke_verification(operation),
        Err(ObservationFailure::Busy)
    ));
    assert_eq!(
        lease.still_valid(now),
        Err(crate::evidence_slot::SlotError::Revoked)
    );
    drop(guard);
    let mut guard = registry.checkout(operation).unwrap();
    let step = guard.operation_mut().actor.run_once();
    let Step::Advanced {
        effects: planned, ..
    } = &step
    else {
        panic!("ожидался отзыв: {step:?}")
    };
    let destroy = *planned
        .iter()
        .find(|effect| effect.spec == EffectSpec::DestroyEvidence)
        .unwrap();
    drop(guard);
    let owner = crate::effect_owner::EffectOwner::default();
    owner.accept_step(operation, &step).unwrap();
    assert_eq!(
        owner.execute_destroy_evidence(&registry, operation, destroy),
        Err(ObservationFailure::Busy)
    );
    assert!(
        registry
            .checkout(operation)
            .unwrap()
            .operation_mut()
            .actor
            .state()
            .body
            .verification()
            .unwrap()
            .evidence
            .is_some()
    );
    drop(lease);
    // The transmission failure after the transition does not repeat either the reception of
    // the material or the receipt: the next holder takes the saved result.
    registry.destroy_actor_evidence(operation, destroy).unwrap();
    owner
        .execute_destroy_evidence(&registry, operation, destroy)
        .unwrap();
    assert!(
        registry
            .checkout(operation)
            .unwrap()
            .operation_mut()
            .actor
            .state()
            .body
            .verification()
            .unwrap()
            .evidence
            .is_none()
    );
    // The untransferred previous step is not replaced, the new one is stored in the register.
    assert!(matches!(
        registry.handoff_evidence_destroy(operation, &owner),
        Err(crate::effect_owner::EffectOwnerError::Duplicate)
    ));
    let deadline = registry
        .checkout(operation)
        .unwrap()
        .operation_mut()
        .actor
        .state()
        .common
        .context
        .deadline;
    assert!(matches!(
        registry.evidence.hold(
            evidence,
            operation,
            deadline,
            zeroize::Zeroizing::new(vec![9]),
            now
        ),
        Err(crate::evidence_slot::SlotError::Unavailable)
    ));
    owner.acknowledge_step(operation).unwrap();
    for effect in planned.iter().copied().filter(|effect| *effect != destroy) {
        owner.claim_effect(operation, effect).unwrap().acknowledge();
    }
    registry
        .handoff_evidence_destroy(operation, &owner)
        .unwrap();
    let next = registry
        .evidence
        .hold(
            evidence,
            operation,
            deadline,
            zeroize::Zeroizing::new(vec![9]),
            now,
        )
        .unwrap();
    assert_ne!(next, reference);
    assert_eq!(
        registry.evidence.invalidate(reference),
        Err(crate::evidence_slot::SlotError::WrongReference)
    );
    assert!(matches!(owner.step(operation), Some(Step::Advanced { .. })));
    assert!(
        registry
            .handoff_evidence_destroy(operation, &owner)
            .is_err()
    );
}

fn idempotent_attempt(
    registry: &OperationRegistry,
    binding: Option<&TrustedCallerBinding>,
    key: u8,
    hint: &str,
    capacity: usize,
) -> IdempotentAdmission {
    idempotent_attempt_at(registry, binding, key, hint, capacity, ConnectionHandle(1))
}

fn idempotent_attempt_at(
    registry: &OperationRegistry,
    binding: Option<&TrustedCallerBinding>,
    key: u8,
    hint: &str,
    capacity: usize,
    connection: ConnectionHandle,
) -> IdempotentAdmission {
    idempotent_attempt_numbered(
        registry,
        binding,
        u16::from(key),
        hint,
        capacity,
        connection,
    )
}

fn idempotent_attempt_numbered(
    registry: &OperationRegistry,
    binding: Option<&TrustedCallerBinding>,
    key: u16,
    hint: &str,
    capacity: usize,
    connection: ConnectionHandle,
) -> IdempotentAdmission {
    use carosign_runtime::authority::verification::{
        PurposeHint, RequestedSubject, VerificationIntent,
    };
    let mut state = carosign_engine::tests_support::initial();
    let id = if let Ok(key) = u8::try_from(key) {
        [key; 16]
    } else {
        let mut id = [0; 16];
        id[14..].copy_from_slice(&key.to_be_bytes());
        id
    };
    state.common.context.operation_id = OperationId::from_bytes(id).unwrap();
    let intent = VerificationIntent {
        report_matched_enrollment: false,
        enrollment: None,
        subject: RequestedSubject::AccountHint(hint.into()),
        purpose_hint: PurposeHint::SessionLogin,
        authority_request_ref: vec![9],
        authenticator: state.common.context.authenticator.id,
    };
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    registry.reserve_idempotent_verification(
        binding,
        IdempotencyKey::from_bytes(id).unwrap(),
        &intent,
        OperationActor::new(state),
        grant,
        &caller,
        capacity,
        connection,
    )
}

#[test]
fn socket_disconnect_reaches_checked_out_owner_without_reading_packets() {
    use std::os::fd::AsRawFd;
    let registry = std::sync::Arc::new(OperationRegistry::new());
    let owner = registry.open_connection(4).unwrap();
    assert!(registry.connection_is_attached(owner));
    let binding = TrustedCallerBinding([1; 32]);
    let IdempotentAdmission::Created(id) =
        idempotent_attempt_at(&registry, Some(&binding), 1, "a", 8, owner)
    else {
        panic!("операция не создана")
    };
    let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let watch =
        crate::server::OwnerDisconnectWatch::start(&sockets.platform, registry.clone(), owner)
            .unwrap();
    let bytes = [42u8];
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(sockets.worker.as_raw_fd(), bytes.as_ptr().cast(), 1, 0) },
        1
    );
    let mut buffer = [0u8; 1];
    // SAFETY: The descriptor and buffer remain valid throughout the read call.
    assert_eq!(
        unsafe {
            libc::recv(
                sockets.platform.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                1,
                0,
            )
        },
        1
    );
    assert_eq!(buffer, bytes);
    let guard = registry.checkout(id).unwrap();
    assert!(!guard.delivery_invalidated());
    drop(sockets.worker);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !guard.delivery_invalidated() {
        assert!(std::time::Instant::now() < deadline, "разрыв не обнаружен");
        std::thread::yield_now();
    }
    assert!(!registry.connection_is_attached(owner));
    assert_eq!(
        idempotent_attempt_at(&registry, Some(&binding), 1, "a", 8, owner),
        IdempotentAdmission::Unavailable
    );
    let other_slots: Vec<_> = (1..DEFAULT_CONNECTIONS)
        .map(|_| registry.open_connection(4).unwrap())
        .collect();
    assert!(registry.open_connection(4).is_none());
    for slot in other_slots {
        registry.close_connection(slot);
    }
    drop(guard);
    let mut guard = registry.checkout_for_control(id).unwrap();
    assert!(guard.operation_mut().actor.has_control_work());
    assert!(matches!(guard.operation_mut().actor.run_once(),
            carosign_runtime::operation::actor::Step::Advanced { ref state, .. }
                if state.common.cancel_requested));
    drop(watch);
    registry.close_connection(owner);
}

#[test]
fn owner_disconnect_is_retained_while_actor_is_checked_out() {
    let registry = OperationRegistry::new();
    let owner = registry.open_connection(4).unwrap();
    let observer = registry.open_connection(4).unwrap();
    let binding = TrustedCallerBinding([1; 32]);
    let IdempotentAdmission::Created(id) =
        idempotent_attempt_at(&registry, Some(&binding), 1, "a", 8, owner)
    else {
        panic!("операция не создана")
    };
    assert_eq!(
        idempotent_attempt_at(&registry, Some(&binding), 1, "a", 8, observer),
        IdempotentAdmission::Existing(id)
    );
    let guard = registry.checkout(id).unwrap();
    registry.close_connection(observer);
    assert!(!guard.delivery_invalidated());
    registry.close_connection(owner);
    assert!(guard.delivery_invalidated());
    assert!(matches!(
        registry.checkout(id),
        Err(ObservationFailure::Busy)
    ));
    drop(guard);
    let mut guard = registry.checkout_for_control(id).unwrap();
    assert!(guard.operation_mut().actor.has_control_work());
    assert!(matches!(
        guard.operation_mut().actor.run_once(),
        carosign_runtime::operation::actor::Step::Advanced { ref state, .. }
            if state.common.cancel_requested
    ));
}

#[test]
fn owner_disconnect_is_queued_for_idle_actor_and_not_repeated() {
    let registry = OperationRegistry::new();
    let owner = registry.open_connection(4).unwrap();
    let binding = TrustedCallerBinding([1; 32]);
    let IdempotentAdmission::Created(id) =
        idempotent_attempt_at(&registry, Some(&binding), 1, "a", 8, owner)
    else {
        panic!("операция не создана")
    };
    registry.close_connection(owner);
    registry.close_connection(owner);
    let mut guard = registry.checkout_for_control(id).unwrap();
    assert!(guard.delivery_invalidated());
    assert!(guard.operation_mut().actor.has_control_work());
    assert!(matches!(
        guard.operation_mut().actor.run_once(),
        carosign_runtime::operation::actor::Step::Advanced { ref state, .. }
            if state.common.cancel_requested
    ));
}

#[test]
fn idempotency_admission_keeps_replay_conflict_and_capacity_distinct() {
    let registry = OperationRegistry::new();
    assert_eq!(registry.open_connection(4), Some(ConnectionHandle(1)));
    let binding = TrustedCallerBinding([1; 32]);
    assert_eq!(
        idempotent_attempt(&registry, None, 1, "a", 1),
        IdempotentAdmission::Unavailable
    );
    let IdempotentAdmission::Created(id) = idempotent_attempt(&registry, Some(&binding), 1, "a", 1)
    else {
        panic!("операция не создана")
    };
    assert_eq!(
        idempotent_attempt(&registry, Some(&binding), 1, "a", 1),
        IdempotentAdmission::Existing(id)
    );
    assert_eq!(
        idempotent_attempt(&registry, Some(&binding), 1, "b", 1),
        IdempotentAdmission::Conflict
    );
    assert_eq!(
        idempotent_attempt(&registry, Some(&binding), 2, "a", 1),
        IdempotentAdmission::Limit
    );
    assert_eq!(
        idempotent_attempt(&registry, Some(&TrustedCallerBinding([2; 32])), 1, "a", 1),
        IdempotentAdmission::Limit
    );
    assert_eq!(registry.entries.lock().unwrap().len(), 1);
}

#[test]
fn idempotent_admission_rejects_grant_from_another_context() {
    use carosign_core::ids::{AuthorityId, BootId, GrantId};
    use carosign_runtime::authority::verification::{
        PurposeHint, RequestedSubject, VerificationIntent,
    };

    let registry = OperationRegistry::new();
    let connection = registry.open_connection(4).unwrap();
    let binding = TrustedCallerBinding([1; 32]);
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    for mismatch in 0..4 {
        let mut state = carosign_engine::tests_support::initial();
        if mismatch == 3 {
            state.common.phase = carosign_engine::OperationPhase::Running;
        }
        let context = &state.common.context;
        let intent = VerificationIntent {
            report_matched_enrollment: false,
            enrollment: None,
            subject: RequestedSubject::Current,
            purpose_hint: PurposeHint::SessionLogin,
            authority_request_ref: vec![],
            authenticator: context.authenticator.id,
        };
        let grant = ValidatedGrant::issue_for_port(
            if mismatch == 0 {
                GrantId::from_bytes([98; 16]).unwrap()
            } else {
                context.owner.binding
            },
            if mismatch == 1 {
                AuthorityId::from_bytes([98; 16]).unwrap()
            } else {
                context.owner.authority
            },
            state.required_grant_scope(),
            if mismatch == 2 {
                BootId::from_bytes([98; 16]).unwrap()
            } else {
                context.deadline.boot()
            },
            context.deadline.instant_ns(),
        );
        assert_eq!(
            registry.reserve_idempotent_verification(
                Some(&binding),
                IdempotencyKey::from_bytes([1; 16]).unwrap(),
                &intent,
                OperationActor::new(state),
                grant,
                &caller,
                4,
                connection,
            ),
            IdempotentAdmission::Unavailable
        );
    }
    let entries = registry.entries.lock().unwrap();
    assert!(entries.operations.is_empty());
    assert!(entries.idempotency.is_empty());
    assert!(entries.connections[&connection].operations.is_empty());
}

#[test]
fn simultaneous_idempotent_requests_publish_one_actor() {
    let registry = std::sync::Arc::new(OperationRegistry::new());
    assert_eq!(registry.open_connection(4), Some(ConnectionHandle(1)));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let registry = registry.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                idempotent_attempt(&registry, Some(&TrustedCallerBinding([1; 32])), 1, "a", 8)
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, IdempotentAdmission::Created(_)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, IdempotentAdmission::Existing(_)))
            .count(),
        7
    );
    let entries = registry.entries.lock().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries.idempotency.len(), 1);
}

#[test]
fn completed_verifications_exhaust_registry_but_keep_idempotency() {
    let registry = OperationRegistry::new();
    let connection = registry.open_connection(1).unwrap();
    let binding = TrustedCallerBinding([1; 32]);
    let attempt =
        |key| idempotent_attempt_numbered(&registry, Some(&binding), key, "a", 300, connection);
    let mut first = None;
    for key in 1..=OPERATIONS_MAX as u16 {
        let IdempotentAdmission::Created(id) = attempt(key) else {
            panic!("не создана операция {key}")
        };
        first.get_or_insert(id);
        let mut guard = registry.checkout(id).unwrap();
        let mut state = guard.operation_mut().actor.state().clone();
        state.common.phase = carosign_engine::OperationPhase::Terminal;
        state.common.terminal = Some(carosign_engine::TerminalOutcome::NoSample);
        guard.operation_mut().actor = OperationActor::new(state);
        guard.operation_mut().cleanup_stage = CleanupStage::Released;
        mark_fixture_recorded(&mut guard);
    }
    assert_eq!(attempt(257), IdempotentAdmission::Limit);
    assert_eq!(attempt(1), IdempotentAdmission::Existing(first.unwrap()));
    assert_eq!(
        registry.entries.lock().unwrap().idempotency.len(),
        OPERATIONS_MAX
    );
}

#[test]
fn completed_operations_release_capacity_without_repeating_retained_keys() {
    use carosign_core::operation::context::BootInstant;
    use carosign_engine::{OperationPhase, TerminalOutcome};
    use carosign_runtime::operation::events::EventKind;

    let registry = OperationRegistry::new();
    let connection = registry.open_connection(1).unwrap();
    let binding = TrustedCallerBinding([1; 32]);
    let deadline = carosign_engine::tests_support::initial()
        .common
        .context
        .deadline;
    let now = BootInstant::new(deadline.boot(), 1);
    let attempt = |key, hint| {
        idempotent_attempt_numbered(&registry, Some(&binding), key, hint, 4096, connection)
    };
    let mut first = None;
    for key in 1..=512 {
        registry.retire_completed(now);
        let IdempotentAdmission::Created(id) = attempt(key, "a") else {
            panic!("operation {key} was not admitted");
        };
        first.get_or_insert(id);
        let mut guard = registry.checkout(id).unwrap();
        let mut state = guard.operation_mut().actor.state().clone();
        state.common.phase = OperationPhase::Terminal;
        state.common.terminal = Some(TerminalOutcome::NoSample);
        guard.operation_mut().actor = OperationActor::new(state);
        guard.operation_mut().cleanup_stage = CleanupStage::Released;
        mark_fixture_recorded(&mut guard);
        registry
            .event_hub(id)
            .unwrap()
            .lock()
            .unwrap()
            .publish(OperationPhase::Terminal, EventKind::Terminal)
            .unwrap();
    }
    let first = first.unwrap();
    assert_eq!(attempt(1, "a"), IdempotentAdmission::Existing(first));
    assert_eq!(attempt(1, "b"), IdempotentAdmission::Conflict);
    assert_eq!(
        registry.live_status(first).unwrap().verification_outcome,
        Some(TerminalOutcome::NoSample)
    );
    assert!(registry.event_hub(first).is_err());

    registry.retire_completed(BootInstant::new(
        deadline.boot(),
        deadline.instant_ns() + 301_000_000_000,
    ));
    assert!(registry.live_status(first).is_none());
    assert!(matches!(attempt(1, "a"), IdempotentAdmission::Created(_)));
}

#[test]
fn retained_idempotency_prevents_reuse_of_an_absent_actor_id() {
    let registry = OperationRegistry::new();
    let connection = registry.open_connection(4).unwrap();
    let original = TrustedCallerBinding([1; 32]);
    let other = TrustedCallerBinding([2; 32]);
    let attempt = |binding: &TrustedCallerBinding, hint| {
        idempotent_attempt_numbered(&registry, Some(binding), 1, hint, 8, connection)
    };
    let IdempotentAdmission::Created(id) = attempt(&original, "a") else {
        panic!("операция не создана")
    };
    // We're just modeling the absence of an actor. It's not an affirmation of cleaning, it's
    // not an implementation of the release of ownership in the workflow.
    {
        let mut entries = registry.entries.lock().unwrap();
        entries.operations.remove(&id).unwrap();
        entries
            .connections
            .get_mut(&connection)
            .unwrap()
            .operations
            .remove(&id);
    }
    assert_eq!(attempt(&original, "a"), IdempotentAdmission::Unavailable);
    assert_eq!(attempt(&original, "b"), IdempotentAdmission::Conflict);
    assert_eq!(attempt(&other, "a"), IdempotentAdmission::Unavailable);
    let entries = registry.entries.lock().unwrap();
    assert!(entries.operations.is_empty());
    assert_eq!(entries.idempotency.len(), 1);
}

#[test]
fn local_retirement_check_keeps_observers_and_outstanding_hub_owners() {
    use carosign_runtime::operation::events::EventKind;
    let registry = OperationRegistry::new();
    let connection = registry.open_connection(1).unwrap();
    let binding = TrustedCallerBinding([1; 32]);
    let IdempotentAdmission::Created(id) =
        idempotent_attempt_numbered(&registry, Some(&binding), 1, "a", 300, connection)
    else {
        panic!("операция не создана")
    };
    {
        let mut guard = registry.checkout(id).unwrap();
        let mut state = guard.operation_mut().actor.state().clone();
        state.common.phase = carosign_engine::OperationPhase::Terminal;
        state.common.terminal = Some(carosign_engine::TerminalOutcome::NoSample);
        guard.operation_mut().actor = OperationActor::new(state);
        guard.operation_mut().cleanup_stage = CleanupStage::Released;
        mark_fixture_recorded(&mut guard);
    }
    assert!(!registry.retirement_locally_drained(id));
    let hub = registry.event_hub(id).unwrap();
    {
        let mut hub = hub.lock().unwrap();
        hub.attach(9, 0).unwrap();
        hub.publish(
            carosign_engine::OperationPhase::Terminal,
            EventKind::Terminal,
        )
        .unwrap();
    }
    assert!(!registry.retirement_locally_drained(id));
    assert!(hub.lock().unwrap().detach(9));
    assert!(!registry.retirement_locally_drained(id));
    drop(hub);
    assert!(registry.retirement_locally_drained(id));
    let saved = {
        let mut entries = registry.entries.lock().unwrap();
        let key = *entries.idempotency.keys().next().unwrap();
        (key, entries.idempotency.remove(&key).unwrap())
    };
    assert!(!registry.retirement_locally_drained(id));
    registry
        .entries
        .lock()
        .unwrap()
        .idempotency
        .insert(saved.0, saved.1);
    assert!(registry.retirement_locally_drained(id));
    // Local inspection does not remove the operation and does not issue authority.
    assert_eq!(
        idempotent_attempt_numbered(&registry, Some(&binding), 1, "a", 300, connection),
        IdempotentAdmission::Existing(id)
    );
}

#[test]
fn connection_slots_follow_publication_replay_terminal_and_close() {
    let registry = OperationRegistry::new();
    let binding = TrustedCallerBinding([1; 32]);
    assert_eq!(registry.open_connection(0), None);
    assert_eq!(registry.open_connection(5), None);
    assert_eq!(registry.open_connection(1), Some(ConnectionHandle(1)));
    assert_eq!(registry.open_connection(1), Some(ConnectionHandle(2)));
    assert_eq!(
        idempotent_attempt(&registry, None, 1, "a", 8),
        IdempotentAdmission::Unavailable
    );
    let IdempotentAdmission::Created(id) = idempotent_attempt(&registry, Some(&binding), 1, "a", 8)
    else {
        panic!("операция не создана")
    };
    assert_eq!(
        idempotent_attempt(&registry, Some(&binding), 1, "a", 8),
        IdempotentAdmission::Existing(id)
    );
    assert_eq!(
        idempotent_attempt(&registry, Some(&binding), 2, "b", 8),
        IdempotentAdmission::Limit
    );
    assert_eq!(
        idempotent_attempt_at(&registry, Some(&binding), 1, "a", 8, ConnectionHandle(2)),
        IdempotentAdmission::Existing(id)
    );
    assert_eq!(
        idempotent_attempt_at(&registry, Some(&binding), 2, "b", 8, ConnectionHandle(2)),
        IdempotentAdmission::Limit
    );
    {
        let mut guard = registry.checkout(id).unwrap();
        let mut state = guard.operation_mut().actor.state().clone();
        state.common.phase = carosign_engine::OperationPhase::Terminal;
        state.common.terminal = Some(carosign_engine::TerminalOutcome::NoSample);
        guard.operation_mut().actor = OperationActor::new(state);
    }
    // The phase without cleaning confirmation and recording does not free up space.
    assert_eq!(
        idempotent_attempt(&registry, Some(&binding), 2, "b", 8),
        IdempotentAdmission::Limit
    );
    {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().cleanup_stage = CleanupStage::Released;
        mark_fixture_recorded(&mut guard);
    }
    assert_eq!(
        idempotent_attempt(&registry, Some(&binding), 1, "a", 8),
        IdempotentAdmission::Existing(id)
    );
    assert!(matches!(
        idempotent_attempt(&registry, Some(&binding), 2, "b", 8),
        IdempotentAdmission::Created(_)
    ));
    registry.close_connection(ConnectionHandle(1));
    assert_eq!(registry.open_connection(1), Some(ConnectionHandle(3)));
    assert_eq!(
        idempotent_attempt(&registry, Some(&binding), 1, "a", 8),
        IdempotentAdmission::Unavailable
    );
    assert!(
        registry
            .entries
            .lock()
            .unwrap()
            .operations
            .contains_key(&id)
    );
}

#[test]
fn terminal_operation_keeps_connection_slot_while_control_is_pending() {
    let registry = OperationRegistry::new();
    let binding = TrustedCallerBinding([1; 32]);
    registry.open_connection(1).unwrap();
    let IdempotentAdmission::Created(id) = idempotent_attempt(&registry, Some(&binding), 1, "a", 8)
    else {
        panic!("операция не создана")
    };
    {
        let mut guard = registry.checkout(id).unwrap();
        let owned = guard.operation_mut();
        let mut state = owned.actor.state().clone();
        state.common.phase = carosign_engine::OperationPhase::Terminal;
        state.common.terminal = Some(carosign_engine::TerminalOutcome::NoSample);
        owned.actor = OperationActor::new(state);
        owned.cleanup_stage = CleanupStage::Released;
        owned
            .actor
            .post(carosign_runtime::OperationMsg::OwnerDetached)
            .unwrap();
        mark_fixture_recorded(&mut guard);
    }
    assert_eq!(
        idempotent_attempt(&registry, Some(&binding), 2, "b", 8),
        IdempotentAdmission::Limit
    );
    {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().actor.run_once();
    }
    assert!(matches!(
        idempotent_attempt(&registry, Some(&binding), 2, "b", 8),
        IdempotentAdmission::Created(_)
    ));
}

#[test]
fn concurrent_distinct_requests_never_exceed_connection_limit() {
    let registry = std::sync::Arc::new(OperationRegistry::new());
    assert_eq!(registry.open_connection(4), Some(ConnectionHandle(1)));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (1..=8)
        .map(|key| {
            let registry = registry.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                idempotent_attempt(
                    &registry,
                    Some(&TrustedCallerBinding([1; 32])),
                    key,
                    "a",
                    16,
                )
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, IdempotentAdmission::Created(_)))
            .count(),
        4
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, IdempotentAdmission::Limit))
            .count(),
        4
    );
    assert_eq!(
        registry.entries.lock().unwrap().connections[&ConnectionHandle(1)]
            .operations
            .len(),
        4
    );
}

#[test]
fn concurrent_connections_do_not_exceed_default() {
    let registry = std::sync::Arc::new(OperationRegistry::new());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(DEFAULT_CONNECTIONS * 2));
    let threads: Vec<_> = (0..DEFAULT_CONNECTIONS * 2)
        .map(|_| {
            let registry = registry.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                registry.open_connection(4)
            })
        })
        .collect();
    let accepted: Vec<_> = threads
        .into_iter()
        .filter_map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(accepted.len(), DEFAULT_CONNECTIONS);
    assert_eq!(
        registry.entries.lock().unwrap().connections.len(),
        DEFAULT_CONNECTIONS
    );
    registry.close_connection(accepted[0]);
    assert!(registry.open_connection(4).is_some());
    assert!(registry.open_connection(4).is_none());
}

#[test]
fn connection_handle_exhaustion_fails_closed_without_reuse() {
    let registry = OperationRegistry::new();
    registry.entries.lock().unwrap().next_connection = u128::MAX;
    assert_eq!(registry.open_connection(1), None);
    assert!(registry.entries.lock().unwrap().connections.is_empty());
}

use mock_reply::MockReply;

use mock_cleanup_broker::MockCleanupBroker;

fn mock_reply(
    reply: MockReply,
    scope: &CleanupScope,
) -> carosign_runtime::ports::BoxFuture<'_, Result<CleanupScope, CleanupPortFailure>> {
    Box::pin(async move {
        match reply {
            MockReply::Same => Ok(scope.clone()),
            MockReply::Unavailable => Err(CleanupPortFailure::Unavailable),
            MockReply::Pending => std::future::pending().await,
            MockReply::Foreign => {
                let lease = carosign_runtime::ports::ResourceLease::issued_for_fixture(
                    carosign_core::ids::LeaseId::from_bytes([77; 16]).unwrap(),
                    scope.owner(),
                    scope.resource_set().clone(),
                );
                Ok(CleanupScope::of(&lease))
            }
            MockReply::ForeignWorker
            | MockReply::ForeignEpoch
            | MockReply::ForeignResources
            | MockReply::ForeignMode => {
                let mut owner = scope.owner();
                let mut resources = scope.resource_set().clone();
                match reply {
                    MockReply::ForeignWorker => {
                        owner.worker = WorkerInstanceId::from_bytes([76; 16]).unwrap()
                    }
                    MockReply::ForeignEpoch => {
                        owner.authenticator.device_epoch =
                            DeviceEpoch::new(owner.authenticator.device_epoch.value() + 1).unwrap()
                    }
                    MockReply::ForeignResources => {
                        resources = fixture_set(
                            vec![carosign_core::ids::ResourceId::from_bytes([75; 16]).unwrap()],
                            carosign_core::resource::AccessMode::Exclusive,
                        )
                    }
                    MockReply::ForeignMode => {
                        resources = fixture_set(
                            scope.resources().to_vec(),
                            carosign_core::resource::AccessMode::Exclusive,
                        );
                    }
                    _ => unreachable!(),
                }
                let lease = carosign_runtime::ports::ResourceLease::issued_for_fixture(
                    scope.lease_id(),
                    owner,
                    resources,
                );
                Ok(CleanupScope::of(&lease))
            }
        }
    })
}

fn pending_verification_cleanup() -> (
    OperationRegistry,
    OperationId,
    carosign_engine::PlannedEffect,
) {
    use carosign_core::ids::{LeaseId, ResourceId};
    use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, decide};
    use carosign_runtime::ports::{ResourceLease, ResourceOwner};

    let mut state = decide(
        &carosign_engine::tests_support::initial(),
        DomainInput::Authorized,
    )
    .unwrap()
    .next;
    for receipt in [EffectReceipt::LeaseGranted, EffectReceipt::GrantValid] {
        let effect = state.common.pending_effects[0];
        state = decide(&state, DomainInput::Receipt { effect, receipt })
            .unwrap()
            .next;
    }
    state = decide(&state, DomainInput::ProviderNoUsableSample)
        .unwrap()
        .next;
    let effect = *state
        .common
        .pending_effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::ReleaseResources)
        .unwrap();
    let id = state.common.context.operation_id;
    let owner = ResourceOwner {
        operation: id,
        worker: WorkerInstanceId::from_bytes([17; 16]).unwrap(),
        authenticator: state.common.context.authenticator,
    };
    let lease = ResourceLease::issued_for_fixture(
        LeaseId::from_bytes([18; 16]).unwrap(),
        owner,
        fixture_set(
            vec![ResourceId::from_bytes([19; 16]).unwrap()],
            carosign_core::resource::AccessMode::SharedRead,
        ),
    );
    state.common.lease = Some(lease.lease_id());
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let registry = OperationRegistry::new();
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let mut guard = registry
        .reserve(OperationActor::new(state), grant, &caller)
        .unwrap();
    guard.operation_mut().lease = Some(lease);
    guard.operation_mut().lease_matches_request = true;
    drop(guard);
    (registry, id, effect)
}

#[test]
fn scheduled_cleanup_fences_full_lease_before_any_broker_wait() {
    use carosign_core::ids::LeaseId;
    use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
    use carosign_runtime::scheduler::{FenceStage, GrantResult, Priority, Request, Scheduler};
    let (registry, id, effect) = pending_verification_cleanup();
    let (owner, lease, set, boot) = {
        let mut guard = registry.checkout(id).unwrap();
        let owned = guard.operation_mut();
        let lease = owned.lease.as_ref().unwrap();
        let owner = lease.owner();
        let id = lease.lease_id();
        let set = lease.resource_set().clone();
        let mut state = owned.actor.state().clone();
        state.common.worker = Some(carosign_engine::WorkerRef {
            instance: owner.worker,
            device_epoch: owner.authenticator.device_epoch,
        });
        let boot = state.common.context.deadline.boot();
        owned.actor = OperationActor::new(state);
        (owner, id, set, boot)
    };
    let mut scheduler = Scheduler::new();
    scheduler.set_epoch(owner.authenticator.device_epoch);
    let make_request = |owner, set| Request {
        owner,
        set,
        priority: Priority::SystemAuth,
        since: carosign_core::operation::context::BootInstant::new(boot, 0),
        mutating: false,
    };
    assert!(matches!(
        scheduler.acquire(make_request(owner, set.clone()), lease),
        GrantResult::Granted { .. }
    ));
    let held = registry
        .begin_scheduled_cleanup(id, effect, &mut scheduler)
        .unwrap();
    assert_eq!(scheduler.fence_stage(lease), Some(FenceStage::Quiescing));
    let other = carosign_runtime::ports::ResourceOwner {
        operation: OperationId::from_bytes([60; 16]).unwrap(),
        ..owner
    };
    let other_set = ResourceSet::new(vec![ResourceClaim {
        id: set.claims()[0].id,
        mode: AccessMode::SharedRead,
    }])
    .unwrap();
    assert_eq!(
        scheduler.acquire(
            make_request(other, other_set),
            LeaseId::from_bytes([61; 16]).unwrap()
        ),
        GrantResult::Queued
    );
    assert!(
        registry
            .begin_scheduled_cleanup(id, effect, &mut scheduler)
            .is_err()
    );
    drop(held);
    assert!(registry.has_retained_lease(id));
    assert!(scheduler.release(lease).is_empty());
    assert_eq!(scheduler.fence_stage(lease), Some(FenceStage::Quiescing));
}

#[test]
fn confirmed_cleanup_at_promotes_starved_interactive_before_system_request() {
    use carosign_core::ids::{LeaseId, OperationId};
    use carosign_core::operation::context::BootInstant;
    use carosign_runtime::scheduler::{GrantResult, Priority, Request, Scheduler};
    let (registry, id, effect) = pending_verification_cleanup();
    let (owner, lease_id, set, boot) = {
        let mut guard = registry.checkout(id).unwrap();
        let owned = guard.operation_mut();
        let lease = owned.lease.as_ref().unwrap();
        let owner = lease.owner();
        let lease_id = lease.lease_id();
        let set = lease.resource_set().clone();
        let mut state = owned.actor.state().clone();
        state.common.worker = Some(carosign_engine::WorkerRef {
            instance: owner.worker,
            device_epoch: owner.authenticator.device_epoch,
        });
        let boot = state.common.context.deadline.boot();
        owned.actor = OperationActor::new(state);
        (owner, lease_id, set, boot)
    };
    let mut scheduler = Scheduler::new();
    scheduler.set_epoch(owner.authenticator.device_epoch);
    let exclusive_set = fixture_set(
        set.claims().iter().map(|claim| claim.id).collect(),
        carosign_core::resource::AccessMode::Exclusive,
    );
    let request = |owner, priority| Request {
        owner,
        set: set.clone(),
        priority,
        since: BootInstant::new(boot, 0),
        mutating: false,
    };
    assert!(matches!(
        scheduler.acquire(request(owner, Priority::SystemAuth), lease_id),
        GrantResult::Granted { .. }
    ));
    let interactive = carosign_runtime::ports::ResourceOwner {
        operation: OperationId::from_bytes([60; 16]).unwrap(),
        ..owner
    };
    let system = carosign_runtime::ports::ResourceOwner {
        operation: OperationId::from_bytes([62; 16]).unwrap(),
        ..owner
    };
    let interactive_lease = LeaseId::from_bytes([61; 16]).unwrap();
    let system_lease = LeaseId::from_bytes([63; 16]).unwrap();
    assert_eq!(
        scheduler.acquire(
            Request {
                set: exclusive_set.clone(),
                ..request(interactive, Priority::InteractiveManage)
            },
            interactive_lease
        ),
        GrantResult::Queued
    );
    assert_eq!(
        scheduler.acquire(
            Request {
                set: exclusive_set,
                ..request(system, Priority::SystemAuth)
            },
            system_lease
        ),
        GrantResult::Queued
    );
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    let clock = carosign_adapters::clock::StepClock::new(boot);
    clock.advance(30_000_000_001);
    let scheduler = Mutex::new(scheduler);
    let progress = futures_executor::block_on(
        registry.cleanup_verification_scheduled_at(id, effect, &scheduler, &broker, &clock),
    )
    .unwrap();
    assert!(matches!(progress.progress, CleanupProgress::Confirmed(_)));
    let now = BootInstant::new(boot, 30_000_000_001);
    let mut scheduler = scheduler.lock().unwrap();
    assert_eq!(
        scheduler.take_cleanup_grants_at(now),
        vec![(interactive_lease, interactive.operation)]
    );
    assert_eq!(
        scheduler.holder_of(set.claims()[0].id),
        Some(interactive.operation)
    );
}

#[test]
fn scheduled_cleanup_fences_before_await_and_retains_on_interruption() {
    use carosign_core::ids::LeaseId;
    use carosign_runtime::scheduler::{FenceStage, GrantResult, Priority, Request, Scheduler};
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    let (registry, id, effect) = pending_verification_cleanup();
    let (owner, lease, set, boot) = {
        let mut guard = registry.checkout(id).unwrap();
        let owned = guard.operation_mut();
        let lease = owned.lease.as_ref().unwrap();
        let owner = lease.owner();
        let lease_id = lease.lease_id();
        let set = lease.resource_set().clone();
        let mut state = owned.actor.state().clone();
        state.common.worker = Some(carosign_engine::WorkerRef {
            instance: owner.worker,
            device_epoch: owner.authenticator.device_epoch,
        });
        let boot = state.common.context.deadline.boot();
        owned.actor = OperationActor::new(state);
        (owner, lease_id, set, boot)
    };
    let mut initial = Scheduler::new();
    initial.set_epoch(owner.authenticator.device_epoch);
    let request = |owner| Request {
        owner,
        set: set.clone(),
        priority: Priority::SystemAuth,
        since: carosign_core::operation::context::BootInstant::new(boot, 0),
        mutating: false,
    };
    assert!(matches!(
        initial.acquire(request(owner), lease),
        GrantResult::Granted { .. }
    ));
    let scheduler = Mutex::new(initial);
    let broker = MockCleanupBroker::new(MockReply::Pending, MockReply::Same);
    let mut future =
        Box::pin(registry.cleanup_verification_scheduled(id, effect, &scheduler, &broker));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
    assert_eq!(
        scheduler.lock().unwrap().fence_stage(lease),
        Some(FenceStage::Quiescing)
    );
    let other = carosign_runtime::ports::ResourceOwner {
        operation: OperationId::from_bytes([60; 16]).unwrap(),
        ..owner
    };
    assert_eq!(
        scheduler
            .lock()
            .unwrap()
            .acquire(request(other), LeaseId::from_bytes([61; 16]).unwrap()),
        GrantResult::Queued
    );
    assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
    assert_eq!(broker.quiesce_calls.load(Ordering::SeqCst), 1);
    drop(future);
    assert!(registry.has_retained_lease(id));
    assert_eq!(
        scheduler.lock().unwrap().fence_stage(lease),
        Some(FenceStage::Quiescing)
    );
    assert!(scheduler.lock().unwrap().release(lease).is_empty());
    assert!(
        futures_executor::block_on(
            registry.cleanup_verification_scheduled(id, effect, &scheduler, &broker)
        )
        .is_err()
    );
    assert_eq!(broker.quiesce_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn scheduled_cleanup_requires_exact_release_and_retains_queue_results() {
    use carosign_core::ids::LeaseId;
    use carosign_runtime::scheduler::{FenceStage, GrantResult, Priority, Request, Scheduler};
    for (reply, expected_success, change_epoch) in [
        (MockReply::Same, true, false),
        (MockReply::Same, true, true),
        (MockReply::ForeignMode, false, false),
    ] {
        let (registry, id, effect) = pending_verification_cleanup();
        let (owner, lease, set, boot) = {
            let mut guard = registry.checkout(id).unwrap();
            let owned = guard.operation_mut();
            let lease = owned.lease.as_ref().unwrap();
            let owner = lease.owner();
            let lease_id = lease.lease_id();
            let set = lease.resource_set().clone();
            let mut state = owned.actor.state().clone();
            state.common.worker = Some(carosign_engine::WorkerRef {
                instance: owner.worker,
                device_epoch: owner.authenticator.device_epoch,
            });
            let boot = state.common.context.deadline.boot();
            owned.actor = OperationActor::new(state);
            (owner, lease_id, set, boot)
        };
        let mut initial = Scheduler::new();
        initial.set_epoch(owner.authenticator.device_epoch);
        let request = |owner| Request {
            owner,
            set: set.clone(),
            priority: Priority::SystemAuth,
            since: carosign_core::operation::context::BootInstant::new(boot, 0),
            mutating: false,
        };
        assert!(matches!(
            initial.acquire(request(owner), lease),
            GrantResult::Granted { .. }
        ));
        let other = carosign_runtime::ports::ResourceOwner {
            operation: OperationId::from_bytes([60; 16]).unwrap(),
            ..owner
        };
        // The second order is placed in the queue in advance: order is not lost at handoff.
        let mut other_set = set.clone();
        other_set = fixture_set(
            other_set.claims().iter().map(|c| c.id).collect(),
            carosign_core::resource::AccessMode::Exclusive,
        );
        let queued = LeaseId::from_bytes([61; 16]).unwrap();
        assert_eq!(
            initial.acquire(
                Request {
                    set: other_set,
                    ..request(other)
                },
                queued
            ),
            GrantResult::Queued
        );
        let scheduler = Mutex::new(initial);
        let broker = MockCleanupBroker::new(MockReply::Same, reply);
        let result = futures_executor::block_on(
            registry.cleanup_verification_scheduled(id, effect, &scheduler, &broker),
        )
        .unwrap();
        if expected_success {
            assert!(matches!(result.progress, CleanupProgress::Confirmed(_)));
            assert!(!registry.has_retained_lease(id));
            if change_epoch {
                scheduler
                    .lock()
                    .unwrap()
                    .set_epoch(DeviceEpoch::new(2).unwrap());
                assert!(scheduler.lock().unwrap().take_cleanup_grants().is_empty());
                assert!(scheduler.lock().unwrap().take_cleanup_grants().is_empty());
                assert_eq!(
                    scheduler.lock().unwrap().holder_of(set.claims()[0].id),
                    Some(other.operation)
                );
                continue;
            }
            assert_eq!(
                scheduler.lock().unwrap().take_cleanup_grants(),
                vec![(queued, other.operation)]
            );
            assert!(scheduler.lock().unwrap().take_cleanup_grants().is_empty());
        } else {
            assert!(matches!(result.progress, CleanupProgress::Uncertain(_)));
            assert!(registry.has_retained_lease(id));
            assert_eq!(
                scheduler.lock().unwrap().fence_stage(lease),
                Some(FenceStage::Uncertain)
            );
            assert!(scheduler.lock().unwrap().take_cleanup_grants().is_empty());
        }
    }
}

#[test]
fn immediate_cleanup_timeout_keeps_scheduler_fence() {
    use carosign_runtime::scheduler::{FenceStage, GrantResult, Priority, Request, Scheduler};
    let (registry, id, effect) = pending_verification_cleanup();
    let (owner, lease, set, boot) = {
        let mut guard = registry.checkout(id).unwrap();
        let owned = guard.operation_mut();
        let lease = owned.lease.as_ref().unwrap();
        let owner = lease.owner();
        let lease_id = lease.lease_id();
        let set = lease.resource_set().clone();
        let mut state = owned.actor.state().clone();
        state.common.worker = Some(carosign_engine::WorkerRef {
            instance: owner.worker,
            device_epoch: owner.authenticator.device_epoch,
        });
        let boot = state.common.context.deadline.boot();
        owned.actor = OperationActor::new(state);
        (owner, lease_id, set, boot)
    };
    let mut scheduler = Scheduler::new();
    scheduler.set_epoch(owner.authenticator.device_epoch);
    assert!(matches!(
        scheduler.acquire(
            Request {
                owner,
                set,
                priority: Priority::SystemAuth,
                since: carosign_core::operation::context::BootInstant::new(boot, 0),
                mutating: false,
            },
            lease
        ),
        GrantResult::Granted { .. }
    ));
    let scheduler = Mutex::new(scheduler);
    use unreachable_broker::UnreachableBroker;
    let broker = UnreachableBroker;
    let cleanup = crate::cleanup_owner::CleanupOwner::default();
    futures_executor::block_on(cleanup.execute_scheduled(
        &registry,
        id,
        effect,
        &scheduler,
        &broker,
        std::future::ready(()),
    ))
    .unwrap();
    assert!(matches!(
        cleanup.take(id),
        Some(crate::cleanup_owner::CleanupResult::Interrupted)
    ));
    assert_eq!(
        scheduler.lock().unwrap().fence_stage(lease),
        Some(FenceStage::Quiescing)
    );
    assert!(registry.has_retained_lease(id));
    assert!(scheduler.lock().unwrap().release(lease).is_empty());
}

#[test]
fn effect_owner_retains_scheduled_result_until_handoff() {
    use carosign_runtime::scheduler::{GrantResult, Priority, Request, Scheduler};
    let (registry, id, effect) = pending_verification_cleanup();
    let (owner, lease, set, boot, step) = {
        let mut guard = registry.checkout(id).unwrap();
        let owned = guard.operation_mut();
        let lease = owned.lease.as_ref().unwrap();
        let owner = lease.owner();
        let lease_id = lease.lease_id();
        let set = lease.resource_set().clone();
        let mut state = owned.actor.state().clone();
        state.common.worker = Some(carosign_engine::WorkerRef {
            instance: owner.worker,
            device_epoch: owner.authenticator.device_epoch,
        });
        let boot = state.common.context.deadline.boot();
        owned.actor = OperationActor::new(state.clone());
        let step = carosign_runtime::operation::actor::Step::Advanced {
            state: Box::new(state),
            effects: vec![effect],
        };
        (owner, lease_id, set, boot, step)
    };
    let mut scheduler = Scheduler::new();
    scheduler.set_epoch(owner.authenticator.device_epoch);
    assert!(matches!(
        scheduler.acquire(
            Request {
                owner,
                set,
                priority: Priority::SystemAuth,
                since: carosign_core::operation::context::BootInstant::new(boot, 0),
                mutating: false
            },
            lease
        ),
        GrantResult::Granted { .. }
    ));
    let scheduler = Mutex::new(scheduler);
    let effects = crate::effect_owner::EffectOwner::default();
    effects.accept_step(id, &step).unwrap();
    effects.acknowledge_step(id).unwrap();
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    futures_executor::block_on(effects.execute_scheduled_cleanup(
        &registry,
        id,
        effect,
        &scheduler,
        &broker,
        std::future::pending(),
    ))
    .unwrap();
    let claim = effects.claim_cleanup(id).unwrap();
    drop(claim);
    let claim = effects.claim_cleanup(id).unwrap();
    effects.accept_scheduled_cleanup(claim).unwrap();
    assert!(effects.claim_cleanup(id).is_none());
    assert!(matches!(
        effects.step(id),
        Some(carosign_runtime::operation::actor::Step::Advanced { .. })
    ));
    assert!(!registry.has_retained_lease(id));
}

#[test]
fn released_proof_cannot_unfence_another_scheduler_with_same_identifiers() {
    use carosign_runtime::scheduler::cleanup::{confirm_quiesced_for, confirm_released};
    use carosign_runtime::scheduler::{FenceStage, GrantResult, Priority, Request, Scheduler};
    let (registry, id, _) = pending_verification_cleanup();
    let (scope, boot) = {
        let mut guard = registry.checkout(id).unwrap();
        let owned = guard.operation_mut();
        (
            CleanupScope::of(owned.lease.as_ref().unwrap()),
            owned.actor.state().common.context.deadline.boot(),
        )
    };
    let request = || Request {
        owner: scope.owner(),
        set: scope.resource_set().clone(),
        priority: Priority::SystemAuth,
        since: carosign_core::operation::context::BootInstant::new(boot, 0),
        mutating: false,
    };
    let mut a = Scheduler::new();
    let mut b = Scheduler::new();
    a.set_epoch(scope.owner().authenticator.device_epoch);
    b.set_epoch(scope.owner().authenticator.device_epoch);
    assert!(matches!(
        a.acquire(request(), scope.lease_id()),
        GrantResult::Granted { .. }
    ));
    assert!(matches!(
        b.acquire(request(), scope.lease_id()),
        GrantResult::Granted { .. }
    ));
    let held_a = a
        .begin_quiescing(scope.lease_id(), scope.owner(), scope.resource_set())
        .unwrap();
    let held_b = b
        .begin_quiescing(scope.lease_id(), scope.owner(), scope.resource_set())
        .unwrap();
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    let proof = futures_executor::block_on(confirm_quiesced_for(&broker, &scope, &held_a)).unwrap();
    let released = futures_executor::block_on(confirm_released(&broker, &scope, proof)).unwrap();
    assert!(b.finish_quiescing(held_b, released).is_err());
    assert_eq!(b.fence_stage(scope.lease_id()), Some(FenceStage::Quiescing));
    assert_eq!(a.fence_stage(scope.lease_id()), Some(FenceStage::Quiescing));
}

fn watch_cleanup_step() -> (
    std::sync::Arc<OperationRegistry>,
    OperationId,
    std::sync::Arc<crate::control_watch::ControlOwner>,
) {
    use carosign_core::ids::{EvidenceId, LeaseId, ResourceId};
    use carosign_engine::{DomainInput, EffectReceipt, decide};
    use carosign_runtime::ports::{ResourceLease, ResourceOwner};
    use std::sync::Arc;
    use std::time::Duration;

    let mut state = decide(
        &carosign_engine::tests_support::initial(),
        DomainInput::Authorized,
    )
    .unwrap()
    .next;
    for receipt in [EffectReceipt::LeaseGranted, EffectReceipt::GrantValid] {
        let effect = state.common.pending_effects[0];
        state = decide(&state, DomainInput::Receipt { effect, receipt })
            .unwrap()
            .next;
    }
    state = decide(
        &state,
        DomainInput::ProviderMatched(EvidenceId::from_bytes([31; 16]).unwrap()),
    )
    .unwrap()
    .next;
    let id = state.common.context.operation_id;
    let lease = ResourceLease::issued_for_fixture(
        LeaseId::from_bytes([18; 16]).unwrap(),
        ResourceOwner {
            operation: id,
            worker: WorkerInstanceId::from_bytes([17; 16]).unwrap(),
            authenticator: state.common.context.authenticator,
        },
        fixture_set(
            vec![ResourceId::from_bytes([19; 16]).unwrap()],
            carosign_core::resource::AccessMode::SharedRead,
        ),
    );
    state.common.lease = Some(lease.lease_id());
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        state.common.context.deadline.boot(),
    ));
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let registry = Arc::new(OperationRegistry::new());
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let mut guard = registry
        .reserve(OperationActor::new(state), grant, &caller)
        .unwrap();
    guard.operation_mut().lease = Some(lease);
    guard.operation_mut().lease_matches_request = true;
    drop(guard);
    let watcher = Arc::new(crate::control_watch::ControlOwner::default());
    let (handle, sender) = watcher
        .start(Arc::clone(&registry), id, clock, Duration::from_secs(2))
        .unwrap();
    sender.revoke().unwrap();
    handle.wait();
    (registry, id, watcher)
}

#[test]
fn watched_revocation_effects_reach_cleanup_owner_without_losing_other_effects() {
    use crate::cleanup_owner::CleanupResult;
    use crate::effect_owner::EffectOwner;
    use carosign_engine::EffectSpec;
    use carosign_runtime::operation::actor::Step;
    for confirmed in [true, false] {
        let (registry, id, watcher) = watch_cleanup_step();
        let owner = EffectOwner::default();
        owner.accept_control(watcher.claim(id).unwrap()).unwrap();
        assert!(watcher.claim(id).is_none());
        let Step::Advanced { effects, .. } = owner.step(id).unwrap() else {
            panic!("нет шага")
        };
        let cleanup = *effects
            .iter()
            .find(|effect| effect.spec == EffectSpec::ReleaseResources)
            .unwrap();
        let suppression = *effects
            .iter()
            .find(|effect| effect.spec == EffectSpec::SuppressResult)
            .unwrap();
        let broker = MockCleanupBroker::new(
            if confirmed {
                MockReply::Same
            } else {
                MockReply::Unavailable
            },
            MockReply::Same,
        );
        futures_executor::block_on(owner.execute_cleanup(
            &registry,
            id,
            cleanup,
            &broker,
            std::future::pending(),
        ))
        .unwrap();
        let result = owner.claim_cleanup(id).expect("результат сохранён");
        if confirmed {
            assert!(matches!(
                result.result(),
                CleanupResult::Completed(Ok(CleanupProgress::Confirmed(_)))
            ));
            assert!(!registry.has_retained_lease(id));
        } else {
            assert!(matches!(
                result.result(),
                CleanupResult::Completed(Ok(CleanupProgress::Uncertain(_)))
            ));
            assert!(registry.has_retained_lease(id));
        }
        drop(result);
        owner
            .claim_cleanup(id)
            .expect("брошенный результат возвращён")
            .acknowledge();
        assert!(owner.claim_cleanup(id).is_none());
        // The other effect and step event does not disappear after handing over the cleaning.
        drop(owner.claim_effect(id, suppression).unwrap());
        assert!(owner.claim_effect(id, suppression).is_ok());
        assert!(owner.step(id).is_some());
    }
}

#[test]
fn deferred_watch_step_survives_busy_actor_and_full_operation_slot() {
    use carosign_engine::{EffectSpec, OperationPhase};
    use carosign_runtime::operation::actor::Step;
    use std::sync::Arc;
    use std::time::Duration;

    let (registry, id, _) = pending_verification_cleanup();
    let registry = Arc::new(registry);
    let mut busy = registry.checkout(id).unwrap();
    let snapshot = busy.operation_mut().actor.state().clone();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        snapshot.common.context.deadline.boot(),
    ));
    let watcher = Arc::new(crate::control_watch::ControlOwner::default());
    let (handle, sender) = watcher
        .start(Arc::clone(&registry), id, clock, Duration::from_secs(2))
        .unwrap();
    sender.revoke().unwrap();
    handle.wait();
    let owner = crate::effect_owner::EffectOwner::default();
    assert!(matches!(
        owner.accept_deferred_control(&registry, watcher.claim(id).unwrap()),
        Err(DeferredControlFailure::Operation(ObservationFailure::Busy))
    ));
    assert!(watcher.claim(id).is_some());
    drop(busy);

    // While the queue is busy, the actor already makes the control step, but the step itself
    // is held at the operation and is not lost if the owner refuses.
    owner
        .accept_step(
            id,
            &Step::Advanced {
                state: Box::new(snapshot),
                effects: Vec::new(),
            },
        )
        .unwrap();
    assert!(matches!(
        owner.accept_deferred_control(&registry, watcher.claim(id).unwrap()),
        Err(DeferredControlFailure::Effects(
            crate::effect_owner::EffectOwnerError::Duplicate
        ))
    ));
    assert!(matches!(
        registry.checkout(id),
        Err(ObservationFailure::Busy)
    ));
    assert!(watcher.claim(id).is_some());
    owner.acknowledge_step(id).unwrap();

    owner
        .accept_deferred_control(&registry, watcher.claim(id).unwrap())
        .unwrap();
    assert!(watcher.claim(id).is_none());
    assert!(
        matches!(owner.step(id), Some(Step::Advanced { ref state, ref effects })
            if state.common.phase == OperationPhase::Finalizing
                && state.common.cancel_requested
                && effects.iter().any(|effect| effect.spec == EffectSpec::SuppressResult))
    );
    assert!(registry.has_retained_lease(id));
}

#[test]
fn interrupted_effect_is_never_reissued_after_broker_started() {
    use crate::cleanup_owner::CleanupResult;
    use crate::effect_owner::{EffectOwner, EffectOwnerError};
    use carosign_engine::EffectSpec;
    use carosign_runtime::operation::actor::Step;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    let (registry, id, watcher) = watch_cleanup_step();
    let owner = EffectOwner::default();
    owner.accept_control(watcher.claim(id).unwrap()).unwrap();
    let Step::Advanced { effects, .. } = owner.step(id).unwrap() else {
        panic!("нет шага")
    };
    let effect = *effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::ReleaseResources)
        .unwrap();
    let broker = MockCleanupBroker::new(MockReply::Pending, MockReply::Same);
    let mut future =
        Box::pin(owner.execute_cleanup(&registry, id, effect, &broker, std::future::pending()));
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(future);
    let result = owner.claim_cleanup(id).unwrap();
    assert!(matches!(result.result(), CleanupResult::Interrupted));
    drop(result);
    owner.claim_cleanup(id).unwrap().acknowledge();
    assert!(registry.has_retained_lease(id));
    assert!(matches!(
        futures_executor::block_on(owner.execute_cleanup(
            &registry,
            id,
            effect,
            &broker,
            std::future::pending()
        )),
        Err(EffectOwnerError::Uncertain)
    ));
    assert_eq!(broker.quiesce_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn unavailable_cleanup_keeps_lease_and_requests_quarantine_without_success() {
    use carosign_engine::{EffectSpec, TerminalOutcome, TransitionIndex};
    use carosign_runtime::operation::actor::Step;
    let (registry, id, effect) = pending_verification_cleanup();
    let wrong = carosign_engine::PlannedEffect::at(
        EffectSpec::ReleaseResources,
        TransitionIndex::new(effect.transition.value() + 1),
        effect.position,
    );
    assert!(matches!(
        registry.mark_cleanup_uncertain(id, wrong),
        Err(CleanupFailure::EffectMismatch)
    ));
    assert!(registry.has_retained_lease(id));
    let step = registry.mark_cleanup_uncertain(id, effect).unwrap();
    assert!(matches!(step, Step::Advanced { ref state, ref effects }
            if state.common.terminal == Some(TerminalOutcome::Unavailable)
                && effects.iter().any(|item| item.spec == EffectSpec::QuarantineScope)
                && !effects.iter().any(|item| matches!(item.spec, EffectSpec::RecordTerminal | EffectSpec::DeliverOutcome))));
    assert!(registry.has_retained_lease(id));
    assert!(matches!(
        registry.mark_cleanup_uncertain(id, effect),
        Err(CleanupFailure::EffectMismatch)
    ));
}

#[test]
fn uncertain_effect_blocks_matching_scheduler_lease_without_confirming_quarantine() {
    use carosign_core::ids::{LeaseId, ResourceId};
    use carosign_core::operation::context::BootInstant;
    use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
    use carosign_engine::{EffectSpec, OperationPhase};
    use carosign_runtime::scheduler::{GrantResult, Priority, Request, Scheduler};

    let (registry, id, cleanup) = pending_verification_cleanup();
    let step = registry.mark_cleanup_uncertain(id, cleanup).unwrap();
    let quarantine = *match &step {
        carosign_runtime::operation::actor::Step::Advanced { effects, .. } => effects,
        _ => panic!("ожидался переход к карантину"),
    }
    .iter()
    .find(|effect| effect.spec == EffectSpec::QuarantineScope)
    .unwrap();
    let (lease_owner, boot) = {
        let mut guard = registry.checkout(id).unwrap();
        let owned = guard.operation_mut();
        let mut state = owned.actor.state().clone();
        let owner = owned.lease.as_ref().unwrap().owner();
        state.common.worker = Some(carosign_engine::WorkerRef {
            instance: owner.worker,
            device_epoch: owner.authenticator.device_epoch,
        });
        let boot = state.common.context.deadline.boot();
        owned.actor = OperationActor::new(state);
        (owner, boot)
    };
    let epoch = lease_owner.authenticator.device_epoch;
    let resource = ResourceId::from_bytes([19; 16]).unwrap();
    let lease = LeaseId::from_bytes([18; 16]).unwrap();
    let request = |operation| Request {
        owner: carosign_runtime::ports::ResourceOwner {
            operation,
            ..lease_owner
        },
        priority: Priority::SystemAuth,
        set: ResourceSet::new(vec![ResourceClaim {
            id: resource,
            mode: AccessMode::SharedRead,
        }])
        .unwrap(),
        since: BootInstant::new(boot, 0),
        mutating: false,
    };
    let mut scheduler = Scheduler::new();
    scheduler.set_epoch(epoch);
    assert!(matches!(
        scheduler.acquire(request(id), lease),
        GrantResult::Granted { .. }
    ));
    let owner = crate::effect_owner::EffectOwner::default();
    owner.accept_step(id, &step).unwrap();
    {
        let mut guard = registry.checkout(id).unwrap();
        let actor = &mut guard.operation_mut().actor;
        let mut state = actor.state().clone();
        state.common.worker.as_mut().unwrap().instance =
            WorkerInstanceId::from_bytes([47; 16]).unwrap();
        *actor = OperationActor::new(state);
    }
    assert!(matches!(
        owner.block_uncertain_scheduler_lease(&registry, id, quarantine, &mut scheduler),
        Err(ObservationFailure::ContextMismatch)
    ));
    {
        let mut guard = registry.checkout(id).unwrap();
        let actor = &mut guard.operation_mut().actor;
        let mut state = actor.state().clone();
        state.common.worker.as_mut().unwrap().instance =
            WorkerInstanceId::from_bytes([17; 16]).unwrap();
        *actor = OperationActor::new(state);
    }
    let mut foreign_scheduler = Scheduler::new();
    foreign_scheduler.set_epoch(epoch);
    let foreign = OperationId::from_bytes([46; 16]).unwrap();
    assert!(matches!(
        foreign_scheduler.acquire(request(foreign), lease),
        GrantResult::Granted { .. }
    ));
    assert!(matches!(
        owner.block_uncertain_scheduler_lease(&registry, id, quarantine, &mut foreign_scheduler),
        Err(ObservationFailure::ContextMismatch)
    ));
    assert_eq!(foreign_scheduler.holder_of(resource), Some(foreign));
    foreign_scheduler.release(lease);
    assert_eq!(foreign_scheduler.holder_of(resource), None);
    let mut partial_scheduler = Scheduler::new();
    partial_scheduler.set_epoch(epoch);
    let mut partial_request = request(id);
    partial_request.set = ResourceSet::new(vec![ResourceClaim {
        id: ResourceId::from_bytes([48; 16]).unwrap(),
        mode: AccessMode::SharedRead,
    }])
    .unwrap();
    assert!(matches!(
        partial_scheduler.acquire(partial_request, lease),
        GrantResult::Granted { .. }
    ));
    assert!(matches!(
        owner.block_uncertain_scheduler_lease(&registry, id, quarantine, &mut partial_scheduler),
        Err(ObservationFailure::ContextMismatch)
    ));
    partial_scheduler.release(lease);
    assert_eq!(
        partial_scheduler.holder_of(ResourceId::from_bytes([48; 16]).unwrap()),
        None
    );
    // The coincidence of one resource does not prove an exact set of issues.
    let extra = ResourceId::from_bytes([48; 16]).unwrap();
    let mut oversized_scheduler = Scheduler::new();
    oversized_scheduler.set_epoch(epoch);
    let mut oversized_request = request(id);
    oversized_request.set = ResourceSet::new(vec![
        ResourceClaim {
            id: resource,
            mode: AccessMode::SharedRead,
        },
        ResourceClaim {
            id: extra,
            mode: AccessMode::SharedRead,
        },
    ])
    .unwrap();
    assert!(matches!(
        oversized_scheduler.acquire(oversized_request, lease),
        GrantResult::Granted { .. }
    ));
    assert!(matches!(
        owner.block_uncertain_scheduler_lease(&registry, id, quarantine, &mut oversized_scheduler),
        Err(ObservationFailure::ContextMismatch)
    ));
    oversized_scheduler.release(lease);
    assert_eq!(oversized_scheduler.holder_of(resource), None);
    assert_eq!(oversized_scheduler.holder_of(extra), None);

    // The second joint reader must be on his issue, even if holder_of It shows the first one.
    let mut shared_scheduler = Scheduler::new();
    shared_scheduler.set_epoch(epoch);
    let first_lease = LeaseId::from_bytes([49; 16]).unwrap();
    assert!(matches!(
        shared_scheduler.acquire(request(foreign), first_lease),
        GrantResult::Granted { .. }
    ));
    assert!(matches!(
        shared_scheduler.acquire(request(id), lease),
        GrantResult::Granted { .. }
    ));
    assert_eq!(shared_scheduler.holder_of(resource), Some(foreign));
    owner
        .block_uncertain_scheduler_lease(&registry, id, quarantine, &mut shared_scheduler)
        .unwrap();
    assert!(shared_scheduler.release(first_lease).is_empty());
    assert!(shared_scheduler.release(lease).is_empty());
    assert!(matches!(
        shared_scheduler.acquire(request(foreign), first_lease),
        GrantResult::Queued
    ));
    let guard = registry.checkout(id).unwrap();
    assert!(matches!(
        owner.block_uncertain_scheduler_lease(&registry, id, quarantine, &mut scheduler),
        Err(ObservationFailure::Busy)
    ));
    drop(guard);
    let claim = owner.claim_effect(id, quarantine).unwrap();
    assert!(matches!(
        owner.block_uncertain_scheduler_lease(&registry, id, quarantine, &mut scheduler),
        Err(ObservationFailure::Busy)
    ));
    drop(claim);
    owner
        .block_uncertain_scheduler_lease(&registry, id, quarantine, &mut scheduler)
        .unwrap();
    assert!(registry.has_retained_lease(id));
    assert_eq!(registry.phase(id), Some(OperationPhase::Finalizing));
    assert!(owner.claim_effect(id, quarantine).is_ok());
    assert!(scheduler.release(lease).is_empty());
    assert!(matches!(
        scheduler.acquire(
            request(OperationId::from_bytes([44; 16]).unwrap()),
            LeaseId::from_bytes([45; 16]).unwrap()
        ),
        GrantResult::Queued
    ));
    let mut guard = registry.checkout(id).unwrap();
    assert!(guard.operation_mut().actor.pending().contains(&quarantine));
}

#[test]
fn cleanup_does_not_overtake_queued_control() {
    use carosign_runtime::OperationMsg;
    let (registry, id, effect) = pending_verification_cleanup();
    {
        let mut guard = registry.checkout(id).unwrap();
        guard
            .operation_mut()
            .actor
            .post(OperationMsg::GrantRevoked)
            .unwrap();
    }
    assert!(matches!(
        registry.mark_cleanup_uncertain(id, effect),
        Err(CleanupFailure::AwaitingActor)
    ));
    assert!(registry.has_retained_lease(id));
}

#[test]
fn cleanup_releases_only_after_two_matching_broker_responses() {
    use carosign_engine::EffectSpec;
    use carosign_runtime::operation::actor::Step;
    let (registry, id, effect) = pending_verification_cleanup();
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    let result =
        futures_executor::block_on(registry.cleanup_verification(id, effect, &broker)).unwrap();
    assert!(
        matches!(result, CleanupProgress::Confirmed(Step::Advanced { ref state, ref effects })
            if state.common.lease.is_none() && effects.iter().any(|next| next.spec == EffectSpec::RecordTerminal))
    );
    assert_eq!(broker.quiesce_calls.load(Ordering::SeqCst), 1);
    assert_eq!(broker.release_calls.load(Ordering::SeqCst), 1);
    assert!(!registry.has_retained_lease(id));
    assert!(matches!(
        futures_executor::block_on(registry.cleanup_verification(id, effect, &broker)),
        Err(CleanupFailure::NoRetainedLease)
    ));
    assert_eq!(broker.quiesce_calls.load(Ordering::SeqCst), 1);
    assert_eq!(broker.release_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn bounded_driver_moves_cleanup_to_terminal_without_publishing_terminal() {
    use crate::effect_owner::{DriveOutcome, EffectOwner};
    use carosign_runtime::operation::actor::Step;
    let (registry, id, effect) = pending_verification_cleanup();
    let state = {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().actor.state().clone()
    };
    let owner = EffectOwner::default();
    owner
        .accept_step(
            id,
            &Step::Advanced {
                state: Box::new(state),
                effects: vec![effect],
            },
        )
        .unwrap();
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    let result = futures_executor::block_on(owner.drive_negative(
        &registry,
        id,
        &broker,
        std::future::pending,
    ))
    .unwrap();
    assert!(matches!(result, DriveOutcome::TerminalPending));
    assert!(!registry.has_retained_lease(id));
    assert!(
        matches!(owner.step(id), Some(Step::Advanced { ref state, .. }) if state.common.phase == carosign_engine::OperationPhase::Terminal)
    );
    assert_eq!(
        registry
            .event_hub(id)
            .unwrap()
            .lock()
            .unwrap()
            .stream()
            .last_sequence(),
        2
    );
    let boot = {
        let mut guard = registry.checkout(id).unwrap();
        guard
            .operation_mut()
            .actor
            .state()
            .common
            .context
            .deadline
            .boot()
    };
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let clock = carosign_adapters::clock::StepClock::new(boot);
    let prepared = futures_executor::block_on(registry.prepare_owner_verification(
        id,
        &caller,
        &Authority {
            deny: false,
            inject: None,
        },
        &clock,
    ))
    .unwrap();
    assert_eq!(
        prepared.into_parts(),
        (id, carosign_engine::TerminalOutcome::NoSample, 3)
    );
    assert_eq!(
        registry
            .event_hub(id)
            .unwrap()
            .lock()
            .unwrap()
            .stream()
            .last_sequence(),
        3
    );
}

#[test]
fn bounded_driver_retains_quarantine_when_cleanup_is_uncertain() {
    use crate::effect_owner::{DriveOutcome, EffectOwner};
    use carosign_engine::EffectSpec;
    use carosign_runtime::operation::actor::Step;
    let (registry, id, effect) = pending_verification_cleanup();
    let state = {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().actor.state().clone()
    };
    let owner = EffectOwner::default();
    owner
        .accept_step(
            id,
            &Step::Advanced {
                state: Box::new(state),
                effects: vec![effect],
            },
        )
        .unwrap();
    let outcome = futures_executor::block_on(owner.drive_negative(
        &registry,
        id,
        &crate::resources::NoBroker,
        std::future::pending,
    ))
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::ExternalEffect));
    assert!(registry.has_retained_lease(id));
    assert!(
        matches!(owner.step(id), Some(Step::Advanced { ref effects, .. }) if effects.iter().any(|effect| effect.spec == EffectSpec::QuarantineScope))
    );
    let before = registry
        .event_hub(id)
        .unwrap()
        .lock()
        .unwrap()
        .stream()
        .last_sequence();
    let again = futures_executor::block_on(owner.drive_negative(
        &registry,
        id,
        &crate::resources::NoBroker,
        std::future::pending,
    ))
    .unwrap();
    assert!(matches!(again, DriveOutcome::ExternalEffect));
    assert_eq!(
        registry
            .event_hub(id)
            .unwrap()
            .lock()
            .unwrap()
            .stream()
            .last_sequence(),
        before
    );
}

#[test]
fn cleanup_owner_retains_confirmed_and_uncertain_results() {
    use crate::cleanup_owner::{CleanupOwner, CleanupResult};
    use carosign_runtime::operation::actor::Step;
    for success in [true, false] {
        let (registry, id, effect) = pending_verification_cleanup();
        let broker = MockCleanupBroker::new(
            if success {
                MockReply::Same
            } else {
                MockReply::Unavailable
            },
            MockReply::Same,
        );
        let owner = CleanupOwner::default();
        futures_executor::block_on(owner.execute(
            &registry,
            id,
            effect,
            &broker,
            std::future::pending(),
        ))
        .unwrap();
        let result = owner.take(id).expect("результат сохранён");
        if success {
            assert!(matches!(
                result,
                CleanupResult::Completed(Ok(CleanupProgress::Confirmed(Step::Advanced { .. })))
            ));
            assert!(!registry.has_retained_lease(id));
        } else {
            assert!(matches!(
                result,
                CleanupResult::Completed(Ok(CleanupProgress::Uncertain(Step::Advanced { .. })))
            ));
            assert!(registry.has_retained_lease(id));
        }
        assert!(owner.take(id).is_none());
    }
}

#[test]
fn cleanup_step_is_transferred_without_losing_record_or_quarantine_effect() {
    use crate::effect_owner::{EffectOwner, EffectOwnerError};
    use carosign_engine::EffectSpec;
    use carosign_runtime::operation::actor::Step;

    for confirmed in [true, false] {
        let (registry, id, cleanup) = pending_verification_cleanup();
        let step = {
            let mut guard = registry.checkout(id).unwrap();
            Step::Advanced {
                state: Box::new(guard.operation_mut().actor.state().clone()),
                effects: vec![cleanup],
            }
        };
        let owner = EffectOwner::default();
        owner.accept_step(id, &step).unwrap();
        let broker = MockCleanupBroker::new(
            if confirmed {
                MockReply::Same
            } else {
                MockReply::Unavailable
            },
            MockReply::Same,
        );
        futures_executor::block_on(owner.execute_cleanup(
            &registry,
            id,
            cleanup,
            &broker,
            std::future::pending(),
        ))
        .unwrap();
        // The previous step has not yet been passed on to the event owner, and the result of
        // the cleaning should go back to its drawer, not get lost.
        assert_eq!(
            owner.accept_cleanup(owner.claim_cleanup(id).unwrap()),
            Err(EffectOwnerError::Duplicate)
        );
        assert!(owner.claim_cleanup(id).is_some());
        owner.acknowledge_step(id).unwrap();
        owner
            .accept_cleanup(owner.claim_cleanup(id).unwrap())
            .unwrap();
        assert!(owner.claim_cleanup(id).is_none());
        let Step::Advanced { effects, .. } = owner.step(id).unwrap() else {
            panic!("нет сохранённого шага")
        };
        let expected = if confirmed {
            EffectSpec::RecordTerminal
        } else {
            EffectSpec::QuarantineScope
        };
        let next = *effects
            .iter()
            .find(|effect| effect.spec == expected)
            .expect("эффект сохранён");
        // The loss of the holder prior to execution retains the effect for transmission.
        drop(owner.claim_effect(id, next).unwrap());
        assert!(owner.claim_effect(id, next).is_ok());
        assert_eq!(registry.has_retained_lease(id), !confirmed);
    }
}

#[test]
fn mock_cleanup_record_and_authorized_negative_result_reach_seqpacket() {
    use crate::effect_owner::EffectOwner;
    use carosign_engine::{EffectSpec, OperationPhase, TerminalOutcome};
    use carosign_runtime::operation::actor::Step;
    use prost::Message;
    use std::os::fd::AsRawFd;

    let (registry, id, cleanup) = pending_verification_cleanup();
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = carosign_linux::sys::peer_credentials(&pair.platform).unwrap();
    let caller = CallerContext::from_transport(
        [1; 32],
        peer.uid,
        TransactionId::from_bytes([1; 16]).unwrap(),
    );
    let boot = {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().owner = caller.clone();
        guard
            .operation_mut()
            .actor
            .state()
            .common
            .context
            .deadline
            .boot()
    };
    let credentials =
        carosign_linux_ipc::credentials::PeerCredentials::from_kernel(peer.pid, peer.uid, peer.gid)
            .unwrap();
    let connection = crate::server::Connection {
        session: carosign_linux_ipc::client::session::ClientSession::new(credentials, [1; 16]),
        peer,
        fd: pair.platform,
    };
    let initial = {
        let mut guard = registry.checkout(id).unwrap();
        Step::Advanced {
            state: Box::new(guard.operation_mut().actor.state().clone()),
            effects: vec![cleanup],
        }
    };
    let owner = EffectOwner::default();
    owner.accept_step(id, &initial).unwrap();
    owner.acknowledge_step(id).unwrap();
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    futures_executor::block_on(owner.execute_cleanup(
        &registry,
        id,
        cleanup,
        &broker,
        std::future::pending(),
    ))
    .unwrap();
    owner
        .accept_cleanup(owner.claim_cleanup(id).unwrap())
        .unwrap();
    let Step::Advanced { effects, .. } = owner.step(id).unwrap() else {
        panic!("нет шага уборки")
    };
    let record = *effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::RecordTerminal)
        .unwrap();
    owner.acknowledge_step(id).unwrap();
    owner.execute_record(&registry, id, record).unwrap();
    owner.accept_record(id).unwrap();
    assert!(
        matches!(owner.step(id), Some(Step::Advanced { ref state, .. }) if state.common.phase == OperationPhase::Terminal && state.common.terminal == Some(TerminalOutcome::NoSample) && state.common.lease.is_none())
    );
    owner.acknowledge_step(id).unwrap();

    let clock = carosign_adapters::clock::StepClock::new(boot);
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &caller,
            &Authority {
                deny: true,
                inject: None
            },
            &clock
        )),
        Err(OwnerDeliveryFailure::Authority(
            carosign_runtime::ports::AuthorityRefusal::Revoked
        ))
    ));
    let prepared = futures_executor::block_on(registry.prepare_owner_verification(
        id,
        &caller,
        &Authority {
            deny: false,
            inject: None,
        },
        &clock,
    ))
    .unwrap();
    let (operation, outcome, sequence) = prepared.into_parts();
    let bytes = crate::answer::terminal_frame(operation, outcome, sequence).encode_to_vec();
    crate::server::write_frame(&connection, &bytes).unwrap();
    let mut buffer = vec![0; 65536];
    // SAFETY: The descriptor and buffer remain valid throughout the read call.
    let size = unsafe {
        libc::recv(
            pair.worker.as_raw_fd(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            0,
        )
    };
    assert!(size > 0);
    let received =
        carosign_protocol::carosign::client::v1::ClientFrame::decode(&buffer[..size as usize])
            .unwrap();
    assert_eq!(received.operation_id, id.as_bytes());
    assert!(
        matches!(received.payload, Some(carosign_protocol::carosign::client::v1::client_frame::Payload::OperationEvent(event)) if event.sequence == sequence && event.terminal.is_some())
    );
}

#[test]
fn cleanup_owner_interruption_retains_lease_and_marks_result() {
    use crate::cleanup_owner::{CleanupOwner, CleanupResult};
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    for (quiesce, release) in [
        (MockReply::Pending, MockReply::Same),
        (MockReply::Same, MockReply::Pending),
    ] {
        let (registry, id, effect) = pending_verification_cleanup();
        let broker = MockCleanupBroker::new(quiesce, release);
        let owner = CleanupOwner::default();
        let mut future =
            Box::pin(owner.execute(&registry, id, effect, &broker, std::future::pending()));
        assert!(matches!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        assert!(owner.take(id).is_none());
        drop(future);
        assert!(matches!(owner.take(id), Some(CleanupResult::Interrupted)));
        assert!(registry.has_retained_lease(id));
        let result =
            futures_executor::block_on(registry.cleanup_verification(id, effect, &broker)).unwrap();
        assert!(matches!(result, CleanupProgress::Uncertain(_)));
        assert!(registry.has_retained_lease(id));
        assert_eq!(broker.quiesce_calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn confirmed_cleanup_allows_completed_negative_result_for_original_owner() {
    use carosign_engine::{EffectSpec, OperationPhase, TerminalOutcome};
    use carosign_runtime::operation::actor::Step;
    let (registry, id, effect) = pending_verification_cleanup();
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    let result =
        futures_executor::block_on(registry.cleanup_verification(id, effect, &broker)).unwrap();
    let CleanupProgress::Confirmed(Step::Advanced { effects, .. }) = result else {
        panic!("уборка не подтверждена")
    };
    let record = *effects
        .iter()
        .find(|item| item.spec == EffectSpec::RecordTerminal)
        .unwrap();
    let boot = {
        let mut guard = registry.checkout(id).unwrap();
        guard
            .operation_mut()
            .actor
            .state()
            .common
            .context
            .deadline
            .boot()
    };
    assert!(
        matches!(registry.record_negative_verification(id, record), Ok(Step::Advanced { ref state, .. }) if state.common.phase == OperationPhase::Terminal && state.common.lease.is_none())
    );
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let clock = carosign_adapters::clock::StepClock::new(boot);
    let authority = Authority {
        deny: false,
        inject: None,
    };
    let prepared = futures_executor::block_on(
        registry.prepare_owner_verification(id, &caller, &authority, &clock),
    )
    .unwrap();
    assert_eq!(prepared.into_parts(), (id, TerminalOutcome::NoSample, 1));
}

#[test]
fn terminal_record_requires_cleanup_exact_effect_and_is_one_shot() {
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    use carosign_runtime::operation::actor::Step;
    let (registry, id, cleanup) = pending_verification_cleanup();
    let guessed = PlannedEffect::at(EffectSpec::RecordTerminal, TransitionIndex::new(1), 0);
    assert_eq!(
        registry.record_negative_verification(id, guessed),
        Err(TerminalRecordFailure::ContextMismatch)
    );
    assert!(registry.has_retained_lease(id));
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    let CleanupProgress::Confirmed(Step::Advanced { effects, .. }) =
        futures_executor::block_on(registry.cleanup_verification(id, cleanup, &broker)).unwrap()
    else {
        panic!("уборка не подтверждена")
    };
    let record = *effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::RecordTerminal)
        .unwrap();
    let foreign = PlannedEffect::at(
        EffectSpec::RecordTerminal,
        record.transition,
        record.position + 1,
    );
    assert_eq!(
        registry.record_negative_verification(id, foreign),
        Err(TerminalRecordFailure::ContextMismatch)
    );
    assert!(registry.record_negative_verification(id, record).is_ok());
    assert_eq!(
        registry.record_negative_verification(id, record),
        Err(TerminalRecordFailure::AlreadyRecorded)
    );
    assert!(matches!(
        OperationRegistry::new().record_negative_verification(id, record),
        Err(TerminalRecordFailure::OperationUnavailable)
    ));
}

#[test]
fn busy_actor_revocation_survives_terminal_and_blocks_delivery() {
    use carosign_engine::EffectSpec;
    use carosign_runtime::operation::actor::Step;
    let (registry, id, cleanup) = pending_verification_cleanup();
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    let CleanupProgress::Confirmed(Step::Advanced { effects, .. }) =
        futures_executor::block_on(registry.cleanup_verification(id, cleanup, &broker)).unwrap()
    else {
        panic!("уборка не подтверждена")
    };
    let record = *effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::RecordTerminal)
        .unwrap();
    registry.record_negative_verification(id, record).unwrap();
    let guard = registry.checkout(id).unwrap();
    assert!(matches!(
        registry.revoke_verification(id),
        Err(ObservationFailure::Busy)
    ));
    drop(guard);
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let mut guard = registry.checkout(id).unwrap();
    let boot = guard
        .operation_mut()
        .actor
        .state()
        .common
        .context
        .deadline
        .boot();
    let _ = guard.operation_mut().actor.run_once();
    drop(guard);
    let clock = carosign_adapters::clock::StepClock::new(boot);
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &caller,
            &Authority {
                deny: false,
                inject: None
            },
            &clock
        )),
        Err(OwnerDeliveryFailure::Authority(
            carosign_runtime::ports::AuthorityRefusal::Revoked
        ))
    ));
}

#[test]
fn busy_actor_revocation_precedes_terminal_record() {
    use carosign_engine::EffectSpec;
    use carosign_runtime::operation::actor::Step;
    let (registry, id, cleanup) = pending_verification_cleanup();
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    let CleanupProgress::Confirmed(Step::Advanced { effects, .. }) =
        futures_executor::block_on(registry.cleanup_verification(id, cleanup, &broker)).unwrap()
    else {
        panic!("уборка не подтверждена")
    };
    let record = *effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::RecordTerminal)
        .unwrap();
    let guard = registry.checkout(id).unwrap();
    assert!(matches!(
        registry.revoke_verification(id),
        Err(ObservationFailure::Busy)
    ));
    drop(guard);
    assert!(matches!(
        registry.record_negative_verification(id, record),
        Err(TerminalRecordFailure::AwaitingActor)
    ));
    let mut guard = registry.checkout(id).unwrap();
    assert_eq!(
        guard.operation_mut().terminal_record_stage,
        TerminalRecordStage::NotStarted
    );
}

#[test]
fn foreign_or_unavailable_confirmation_keeps_lease_and_never_fakes_cleanup() {
    use carosign_engine::{EffectSpec, TerminalOutcome};
    use carosign_runtime::operation::actor::Step;
    for (quiesce, release, expected_release_calls) in [
        (MockReply::Unavailable, MockReply::Same, 0),
        (MockReply::Foreign, MockReply::Same, 0),
        (MockReply::ForeignWorker, MockReply::Same, 0),
        (MockReply::ForeignEpoch, MockReply::Same, 0),
        (MockReply::ForeignResources, MockReply::Same, 0),
        (MockReply::ForeignMode, MockReply::Same, 0),
        (MockReply::Same, MockReply::Unavailable, 1),
        (MockReply::Same, MockReply::Foreign, 1),
        (MockReply::Same, MockReply::ForeignEpoch, 1),
        (MockReply::Same, MockReply::ForeignMode, 1),
    ] {
        let (registry, id, effect) = pending_verification_cleanup();
        let broker = MockCleanupBroker::new(quiesce, release);
        let result =
            futures_executor::block_on(registry.cleanup_verification(id, effect, &broker)).unwrap();
        assert!(
            matches!(result, CleanupProgress::Uncertain(Step::Advanced { ref state, ref effects })
                if state.common.lease.is_some()
                    && state.common.terminal == Some(TerminalOutcome::Unavailable)
                    && effects.iter().any(|next| next.spec == EffectSpec::QuarantineScope)
                    && !effects.iter().any(|next| matches!(next.spec, EffectSpec::RecordTerminal | EffectSpec::DeliverOutcome)))
        );
        assert!(registry.has_retained_lease(id));
        assert_eq!(broker.quiesce_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            broker.release_calls.load(Ordering::SeqCst),
            expected_release_calls
        );
    }
}

#[test]
fn no_broker_preserves_uncertain_lease() {
    use carosign_runtime::operation::actor::Step;
    let (registry, id, effect) = pending_verification_cleanup();
    let result = futures_executor::block_on(registry.cleanup_verification(
        id,
        effect,
        &crate::resources::NoBroker,
    ))
    .unwrap();
    assert!(matches!(
        result,
        CleanupProgress::Uncertain(Step::Advanced { .. })
    ));
    assert!(registry.has_retained_lease(id));
}

#[test]
fn mismatched_grant_never_reaches_cleanup_broker() {
    use carosign_runtime::operation::actor::Step;
    let (registry, id, effect) = pending_verification_cleanup();
    {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().lease_matches_request = false;
    }
    let broker = MockCleanupBroker::new(MockReply::Same, MockReply::Same);
    let result =
        futures_executor::block_on(registry.cleanup_verification(id, effect, &broker)).unwrap();
    assert!(matches!(
        result,
        CleanupProgress::Uncertain(Step::Advanced { .. })
    ));
    assert!(registry.has_retained_lease(id));
    assert_eq!(broker.quiesce_calls.load(Ordering::SeqCst), 0);
    assert_eq!(broker.release_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn interruption_of_either_broker_call_keeps_lease_and_forbids_automatic_retry() {
    use carosign_engine::EffectSpec;
    use carosign_runtime::operation::actor::Step;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    for (quiesce, release, expected_release_calls) in [
        (MockReply::Pending, MockReply::Same, 0),
        (MockReply::Same, MockReply::Pending, 1),
    ] {
        let (registry, id, effect) = pending_verification_cleanup();
        let broker = MockCleanupBroker::new(quiesce, release);
        let mut future = Box::pin(registry.cleanup_verification(id, effect, &broker));
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        drop(future);
        assert!(registry.has_retained_lease(id));
        let result =
            futures_executor::block_on(registry.cleanup_verification(id, effect, &broker)).unwrap();
        assert!(
            matches!(result, CleanupProgress::Uncertain(Step::Advanced { ref effects, .. })
                if effects.iter().any(|next| next.spec == EffectSpec::QuarantineScope))
        );
        assert!(registry.has_retained_lease(id));
        assert_eq!(broker.quiesce_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            broker.release_calls.load(Ordering::SeqCst),
            expected_release_calls
        );
    }
}

#[test]
fn revocation_during_release_is_processed_before_cleanup_receipt() {
    use carosign_engine::{EffectSpec, TerminalOutcome};
    use carosign_runtime::operation::actor::Step;
    use gate_broker::GateBroker;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    let (registry, id, effect) = pending_verification_cleanup();
    let broker = GateBroker {
        ready: AtomicBool::new(false),
    };
    let mut future = Box::pin(registry.cleanup_verification(id, effect, &broker));
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
    assert!(matches!(
        registry.revoke_verification(id),
        Err(ObservationFailure::Busy)
    ));
    broker.ready.store(true, Ordering::SeqCst);
    let Poll::Ready(Ok(progress)) = future.as_mut().poll(&mut context) else {
        panic!("уборка не завершена")
    };
    assert!(
        matches!(progress, CleanupProgress::Confirmed(Step::Advanced { ref state, ref effects })
            if state.common.terminal == Some(TerminalOutcome::Cancelled)
                && effects.iter().any(|next| next.spec == EffectSpec::SuppressResult)
                && !effects.iter().any(|next| next.spec == EffectSpec::DeliverOutcome))
    );
    drop(future);
    assert!(!registry.has_retained_lease(id));
}

#[test]
fn cleanup_rejects_a_stale_epoch_without_releasing_the_lease() {
    use carosign_core::ids::{LeaseId, ResourceId};
    use carosign_runtime::ports::{ResourceLease, ResourceOwner};
    let (registry, id, effect) = pending_verification_cleanup();
    {
        let mut guard = registry.checkout(id).unwrap();
        let authenticator = guard
            .operation_mut()
            .actor
            .state()
            .common
            .context
            .authenticator;
        guard.operation_mut().lease = Some(ResourceLease::issued_for_fixture(
            LeaseId::from_bytes([18; 16]).unwrap(),
            ResourceOwner {
                operation: id,
                worker: WorkerInstanceId::from_bytes([17; 16]).unwrap(),
                authenticator: carosign_core::device::AuthenticatorRef {
                    id: authenticator.id,
                    device_epoch: DeviceEpoch::new(authenticator.device_epoch.value() + 1).unwrap(),
                },
            },
            fixture_set(
                vec![ResourceId::from_bytes([19; 16]).unwrap()],
                carosign_core::resource::AccessMode::SharedRead,
            ),
        ));
    }
    assert!(matches!(
        registry.mark_cleanup_uncertain(id, effect),
        Err(CleanupFailure::LeaseContextMismatch)
    ));
    assert!(registry.has_retained_lease(id));
}

#[test]
fn deadline_check_keeps_operation_before_deadline_and_posts_at_deadline() {
    let state = carosign_engine::tests_support::initial();
    let id = state.common.context.operation_id;
    let deadline = state.common.context.deadline;
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        deadline.boot(),
        deadline.instant_ns(),
    );
    let registry = OperationRegistry::new();
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    drop(
        registry
            .reserve(OperationActor::new(state), grant, &caller)
            .unwrap(),
    );
    let before = carosign_core::operation::context::BootInstant::new(
        deadline.boot(),
        deadline.instant_ns().saturating_sub(1),
    );
    assert!(
        registry
            .check_verification_deadline(id, before)
            .unwrap()
            .is_none()
    );
    let at =
        carosign_core::operation::context::BootInstant::new(deadline.boot(), deadline.instant_ns());
    assert!(
        registry
            .check_verification_deadline(id, at)
            .unwrap()
            .is_some()
    );
}

#[test]
fn revocation_overtakes_queued_provider_result_and_suppresses_success() {
    use carosign_engine::{
        DomainInput, EffectReceipt, EffectSpec, OperationPhase, TerminalOutcome, decide,
    };
    use carosign_runtime::operation::actor::Step;
    let mut state = decide(
        &carosign_engine::tests_support::initial(),
        DomainInput::Authorized,
    )
    .unwrap()
    .next;
    for receipt in [EffectReceipt::LeaseGranted, EffectReceipt::GrantValid] {
        let effect = state.common.pending_effects[0];
        state = decide(&state, DomainInput::Receipt { effect, receipt })
            .unwrap()
            .next;
    }
    let id = state.common.context.operation_id;
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let mut actor = OperationActor::new(state);
    actor
        .post(carosign_runtime::OperationMsg::Domain(
            DomainInput::ProviderNoUsableSample,
        ))
        .unwrap();
    let registry = OperationRegistry::new();
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    drop(registry.reserve(actor, grant, &caller).unwrap());
    let guard = registry.checkout(id).unwrap();
    assert!(matches!(
        registry.revoke_verification(id),
        Err(ObservationFailure::Busy)
    ));
    drop(guard);
    let mut guard = registry.checkout(id).unwrap();
    let step = guard.operation_mut().actor.run_once();
    assert!(matches!(step, Step::Advanced { ref state, ref effects }
            if state.common.phase == OperationPhase::Cancelling
                && state.common.terminal == Some(TerminalOutcome::Cancelled)
                && effects.iter().any(|effect| effect.spec == EffectSpec::SuppressResult)));
}
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{AuthenticatorId, CommandId, WorkerInstanceId};
use carosign_runtime::ports::TransactionId;

#[test]
fn positive_reservation_is_retained_and_unsent_revocation_unblocks_stream() {
    use carosign_engine::{
        EffectSpec, OperationPhase, PlannedEffect, TerminalOutcome, TransitionIndex,
    };
    use carosign_runtime::operation::events::{EventError, EventKind};

    for disconnect in [false, true] {
        let mut state = carosign_engine::tests_support::initial();
        let id = state.common.context.operation_id;
        let now = carosign_core::operation::context::BootInstant::new(
            state.common.context.deadline.boot(),
            0,
        );
        state.common.phase = OperationPhase::Finalizing;
        state.common.terminal = Some(TerminalOutcome::FactorAccepted);
        let effect = PlannedEffect::at(EffectSpec::DeliverOutcome, TransitionIndex::new(1), 0);
        state.common.pending_effects = vec![effect];
        let grant = ValidatedGrant::issue_for_port(
            state.common.context.owner.binding,
            state.common.context.owner.authority,
            state.required_grant_scope(),
            state.common.context.deadline.boot(),
            state.common.context.deadline.instant_ns(),
        );
        let caller = CallerContext::from_transport(
            [1; 32],
            1000,
            TransactionId::from_bytes([1; 16]).unwrap(),
        );
        let foreign = CallerContext::from_transport(
            [2; 32],
            1000,
            TransactionId::from_bytes([2; 16]).unwrap(),
        );
        let registry = OperationRegistry::new();
        let connection = registry.open_connection(4).unwrap();
        let mut guard = registry
            .reserve(OperationActor::new(state), grant, &caller)
            .unwrap();
        guard.operation_mut().cleanup_stage = CleanupStage::Released;
        drop(guard);
        registry
            .entries
            .lock()
            .unwrap()
            .get_mut(&id)
            .unwrap()
            .owner_connection = Some(connection);
        assert!(matches!(
            registry.reserve_positive_delivery(id, &foreign, effect, now),
            Err(OwnerDeliveryFailure::WrongOwner)
        ));
        assert_eq!(
            registry.reserve_positive_delivery(id, &caller, effect, now),
            Ok(1)
        );
        assert!(matches!(
            registry.reserve_positive_delivery(id, &caller, effect, now),
            Err(OwnerDeliveryFailure::AlreadyReserved)
        ));
        let events = registry.event_hub(id).unwrap();
        assert_eq!(
            events
                .lock()
                .unwrap()
                .publish(OperationPhase::Finalizing, EventKind::StateChanged),
            Err(EventError::Reserved)
        );
        if disconnect {
            registry.detach_connection(connection);
        } else {
            assert!(registry.revoke_verification(id).is_ok());
        }
        assert!(
            events
                .lock()
                .unwrap()
                .publish(OperationPhase::Cancelling, EventKind::StateChanged)
                .is_ok()
        );
        assert!(
            registry
                .reserve_positive_delivery(id, &caller, effect, now)
                .is_err()
        );
    }
}

#[test]
fn revocation_cannot_cancel_started_positive_reservation() {
    use carosign_engine::{
        EffectSpec, OperationPhase, PlannedEffect, TerminalOutcome, TransitionIndex,
    };
    use carosign_runtime::operation::events::{EventError, EventKind};
    use carosign_runtime::operation::hub::ReservedSendOutcome;
    let mut state = carosign_engine::tests_support::initial();
    let id = state.common.context.operation_id;
    let now = carosign_core::operation::context::BootInstant::new(
        state.common.context.deadline.boot(),
        0,
    );
    state.common.phase = OperationPhase::Finalizing;
    state.common.terminal = Some(TerminalOutcome::FactorAccepted);
    let effect = PlannedEffect::at(EffectSpec::DeliverOutcome, TransitionIndex::new(1), 0);
    state.common.pending_effects = vec![effect];
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let registry = OperationRegistry::new();
    let mut guard = registry
        .reserve(OperationActor::new(state), grant, &caller)
        .unwrap();
    guard.operation_mut().cleanup_stage = CleanupStage::Released;
    drop(guard);
    registry
        .reserve_positive_delivery(id, &caller, effect, now)
        .unwrap();
    {
        let entries = registry.entries.lock().unwrap();
        let entry = entries.get(&id).unwrap();
        entry
            .events
            .lock()
            .unwrap()
            .begin_reserved_send(entry.positive_reservation.as_ref().unwrap())
            .unwrap();
    }
    assert!(registry.revoke_verification(id).is_ok());
    let entries = registry.entries.lock().unwrap();
    let entry = entries.get(&id).unwrap();
    let reservation = entry
        .positive_reservation
        .as_ref()
        .expect("начатая запись удерживается");
    let mut hub = entry.events.lock().unwrap();
    hub.finish_reserved_send(reservation, ReservedSendOutcome::Unknown)
        .unwrap();
    assert_eq!(
        hub.publish(OperationPhase::Finalizing, EventKind::StateChanged),
        Err(EventError::Reserved)
    );
    assert!(hub.cancel_reserved_terminal(reservation).is_err());
}

#[test]
fn positive_terminal_record_requires_sent_and_confirmed_cleanup() {
    use carosign_engine::transition::VerificationDelivery;
    use carosign_engine::{
        EffectSpec, OperationPhase, PlannedEffect, TerminalOutcome, TransitionIndex,
    };
    use carosign_runtime::operation::actor::Step;
    let mut state = carosign_engine::tests_support::initial();
    let id = state.common.context.operation_id;
    state.common.phase = OperationPhase::Finalizing;
    state.common.terminal = Some(TerminalOutcome::FactorAccepted);
    state.common.transition_index = TransitionIndex::new(1);
    let effect = PlannedEffect::at(EffectSpec::RecordTerminal, TransitionIndex::new(1), 0);
    state.common.pending_effects = vec![effect];
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let registry = OperationRegistry::new();
    let mut guard = registry
        .reserve(OperationActor::new(state), grant, &caller)
        .unwrap();
    guard.operation_mut().cleanup_stage = CleanupStage::Released;
    drop(guard);
    assert_eq!(
        registry.record_delivered_verification(id, effect),
        Err(TerminalRecordFailure::ContextMismatch)
    );
    let unsent_state = {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().actor.state().clone()
    };
    let unsent_owner = crate::effect_owner::EffectOwner::default();
    unsent_owner
        .accept_step(
            id,
            &Step::Advanced {
                state: Box::new(unsent_state),
                effects: vec![effect],
            },
        )
        .unwrap();
    assert!(matches!(
        futures_executor::block_on(unsent_owner.drive_negative(
            &registry,
            id,
            &crate::resources::NoBroker,
            std::future::pending,
        )),
        Err(crate::effect_owner::DriveError::Effect(
            crate::effect_owner::EffectOwnerError::UnsupportedRecord
        ))
    ));
    assert_eq!(registry.phase(id), Some(OperationPhase::Finalizing));
    let mut guard = registry.checkout(id).unwrap();
    let mut state = guard.operation_mut().actor.state().clone();
    let carosign_engine::OperationBody::Verification(verification) = &mut state.body else {
        unreachable!()
    };
    verification.delivery = VerificationDelivery::Sent;
    guard.operation_mut().actor = OperationActor::new(state);
    drop(guard);
    assert!(matches!(registry.revoke_verification(id),
            Ok(carosign_runtime::operation::actor::Step::Advanced { state, .. })
            if state.common.terminal == Some(TerminalOutcome::FactorAccepted)));
    let state = {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().actor.state().clone()
    };
    let owner = crate::effect_owner::EffectOwner::default();
    owner
        .accept_step(
            id,
            &Step::Advanced {
                state: Box::new(state),
                effects: vec![effect],
            },
        )
        .unwrap();
    assert!(matches!(
        futures_executor::block_on(owner.drive_negative(
            &registry,
            id,
            &crate::resources::NoBroker,
            std::future::pending,
        )),
        Ok(crate::effect_owner::DriveOutcome::TerminalPending)
    ));
    assert!(matches!(owner.step(id), Some(Step::Advanced { state, .. })
            if state.common.phase == OperationPhase::Terminal && state.common.terminal == Some(TerminalOutcome::FactorAccepted)));
    assert_eq!(
        registry.record_delivered_verification(id, effect),
        Err(TerminalRecordFailure::AlreadyRecorded)
    );
}

#[test]
fn positive_publication_requires_record_and_sent_reservation() {
    use carosign_engine::{
        DomainInput, EffectReceipt, EffectSpec, OperationPhase, PlannedEffect, TerminalOutcome,
        TransitionIndex,
    };
    use prost::Message;
    use std::os::fd::AsRawFd;
    let mut state = carosign_engine::tests_support::initial();
    let id = state.common.context.operation_id;
    let now = carosign_core::operation::context::BootInstant::new(
        state.common.context.deadline.boot(),
        0,
    );
    state.common.phase = OperationPhase::Finalizing;
    state.common.terminal = Some(TerminalOutcome::FactorAccepted);
    state.common.transition_index = TransitionIndex::new(1);
    let deliver = PlannedEffect::at(EffectSpec::DeliverOutcome, TransitionIndex::new(1), 0);
    state.common.pending_effects = vec![deliver];
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    let registry = OperationRegistry::new();
    let mut guard = registry
        .reserve(OperationActor::new(state), grant, &caller)
        .unwrap();
    guard.operation_mut().cleanup_stage = CleanupStage::Released;
    drop(guard);
    assert_eq!(
        registry.reserve_positive_delivery(id, &caller, deliver, now),
        Ok(1)
    );
    assert!(registry.publish_delivered_verification(id).is_err());
    // Test source of trust: The synthetic state above is not a validation of the recipient or
    // confirmation of eligibility in the work system.
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let bytes =
        crate::answer::terminal_frame(id, TerminalOutcome::FactorAccepted, 1).encode_to_vec();
    {
        let mut entries = registry.entries.lock().unwrap();
        let entry = entries.get_mut(&id).unwrap();
        assert_eq!(
            send_reserved_positive(entry, &pair.platform, &[]),
            Err(OwnerSendError::Socket(libc::EMSGSIZE))
        );
        let oversized = vec![0; carosign_protocol::limits::FRAME_MAX + 1];
        assert_eq!(
            send_reserved_positive(entry, &pair.platform, &oversized),
            Err(OwnerSendError::Socket(libc::EMSGSIZE))
        );
    }
    let filler = [0u8; 4096];
    let mut queued = 0;
    loop {
        // SAFETY: The descriptor and buffer belong to this test.
        let sent = unsafe {
            libc::send(
                pair.platform.as_raw_fd(),
                filler.as_ptr().cast(),
                filler.len(),
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        if sent < 0 {
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EAGAIN)
            );
            break;
        }
        queued += 1;
        assert!(queued < 10000);
    }
    {
        let mut entries = registry.entries.lock().unwrap();
        let entry = entries.get_mut(&id).unwrap();
        assert_eq!(
            send_reserved_positive(entry, &pair.platform, &bytes),
            Ok(OwnerSendProgress::WouldBlock)
        );
    }
    for _ in 0..queued {
        let mut discarded = [0u8; 4096];
        // SAFETY: The buffer has room for each packet sent above.
        assert_eq!(
            unsafe {
                libc::recv(
                    pair.worker.as_raw_fd(),
                    discarded.as_mut_ptr().cast(),
                    discarded.len(),
                    libc::MSG_DONTWAIT,
                )
            },
            filler.len() as isize
        );
    }
    {
        let mut entries = registry.entries.lock().unwrap();
        let entry = entries.get_mut(&id).unwrap();
        assert_eq!(
            send_reserved_positive(entry, &pair.platform, &bytes),
            Ok(OwnerSendProgress::Sent)
        );
        assert_eq!(
            send_reserved_positive(entry, &pair.platform, &bytes),
            Err(OwnerSendError::Finished)
        );
    }
    let mut received = vec![0u8; bytes.len()];
    // SAFETY: The buffer is allocated to the full length of the received packet.
    let count = unsafe {
        libc::recv(
            pair.worker.as_raw_fd(),
            received.as_mut_ptr().cast(),
            received.len(),
            libc::MSG_DONTWAIT,
        )
    };
    assert_eq!(count, bytes.len() as isize);
    assert_eq!(received, bytes);
    let mut guard = registry.checkout(id).unwrap();
    let actor = &mut guard.operation_mut().actor;
    actor
        .post(carosign_runtime::OperationMsg::Domain(
            DomainInput::Receipt {
                effect: deliver,
                receipt: EffectReceipt::OwnerDeliveryRecorded,
            },
        ))
        .unwrap();
    let step = actor.run_once();
    let carosign_runtime::operation::actor::Step::Advanced { effects, .. } = step else {
        panic!("актор не принял подтверждение доставки")
    };
    let record = *effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::RecordTerminal)
        .unwrap();
    drop(guard);
    registry.record_delivered_verification(id, record).unwrap();
    assert!(registry.publish_delivered_verification(id).is_ok());
    assert!(registry.publish_delivered_verification(id).is_err());
}

fn unavailable_operation() -> (OperationActor, ValidatedGrant, CallerContext) {
    use carosign_engine::{DomainInput, EffectReceipt, decide};
    let mut state = decide(
        &carosign_engine::tests_support::initial(),
        DomainInput::Authorized,
    )
    .unwrap()
    .next;
    let effect = state.common.pending_effects[0];
    state = decide(
        &state,
        DomainInput::Receipt {
            effect,
            receipt: EffectReceipt::LeaseDenied,
        },
    )
    .unwrap()
    .next;
    let effect = state.common.pending_effects[0];
    state = decide(
        &state,
        DomainInput::Receipt {
            effect,
            receipt: EffectReceipt::VerificationTerminalRecorded,
        },
    )
    .unwrap()
    .next;
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    (OperationActor::new(state), grant, caller)
}

use authority::Authority;

fn mark_fixture_recorded(guard: &mut OperationGuard<'_>) {
    let owned = guard.operation_mut();
    let outcome = owned.actor.state().common.terminal.unwrap();
    owned.terminal_record_stage = TerminalRecordStage::Recorded {
        effect: carosign_engine::PlannedEffect::at(
            carosign_engine::EffectSpec::RecordTerminal,
            carosign_engine::TransitionIndex::new(2),
            0,
        ),
        outcome,
    };
}

#[test]
fn owner_delivery_requires_exact_caller_current_grant_and_is_one_shot() {
    use unbound_authority::UnboundAuthority;
    let (actor, grant, caller) = unavailable_operation();
    let id = actor.state().common.context.operation_id;
    let clock =
        carosign_adapters::clock::StepClock::new(actor.state().common.context.deadline.boot());
    let registry = OperationRegistry::new();
    let mut guard = registry.reserve(actor, grant, &caller).unwrap();
    mark_fixture_recorded(&mut guard);
    drop(guard);
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &caller,
            &UnboundAuthority,
            &clock,
        )),
        Err(OwnerDeliveryFailure::Authority(
            carosign_runtime::ports::AuthorityRefusal::Unavailable
        ))
    ));
    let wrong = CallerContext::from_transport([1; 32], 1001, caller.transaction());
    let wrong_transaction =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([2; 16]).unwrap());
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &wrong,
            &Authority {
                deny: false,
                inject: None
            },
            &clock
        )),
        Err(OwnerDeliveryFailure::WrongOwner)
    ));
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &wrong_transaction,
            &Authority {
                deny: false,
                inject: None
            },
            &clock,
        )),
        Err(OwnerDeliveryFailure::WrongOwner)
    ));
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &caller,
            &Authority {
                deny: true,
                inject: None
            },
            &clock
        )),
        Err(OwnerDeliveryFailure::Authority(
            carosign_runtime::ports::AuthorityRefusal::Revoked
        ))
    ));
    {
        let mut guard = registry.checkout(id).unwrap();
        assert!(
            guard
                .operation_mut()
                .events
                .lock()
                .unwrap()
                .publish(
                    carosign_engine::OperationPhase::Running,
                    carosign_runtime::operation::events::EventKind::StateChanged,
                )
                .unwrap()
                .is_empty()
        );
    }
    let prepared = futures_executor::block_on(registry.prepare_owner_verification(
        id,
        &caller,
        &Authority {
            deny: false,
            inject: None,
        },
        &clock,
    ))
    .unwrap();
    assert_eq!(
        prepared.into_parts(),
        (id, carosign_engine::TerminalOutcome::Unavailable, 2)
    );
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &caller,
            &Authority {
                deny: false,
                inject: None
            },
            &clock
        )),
        Err(OwnerDeliveryFailure::AlreadyReserved)
    ));
}

#[test]
fn authorized_negative_result_uses_stream_sequence_and_reaches_seqpacket_peer() {
    use carosign_runtime::operation::events::EventKind;
    use prost::Message;
    use std::os::fd::AsRawFd;
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = carosign_linux::sys::peer_credentials(&pair.platform).unwrap();
    let credentials =
        carosign_linux_ipc::credentials::PeerCredentials::from_kernel(peer.pid, peer.uid, peer.gid)
            .unwrap();
    let session = carosign_linux_ipc::client::session::ClientSession::new(credentials, [1; 16]);
    let mut opaque = [0; 32];
    opaque[..4].copy_from_slice(&peer.pid.to_be_bytes());
    opaque[4..8].copy_from_slice(&peer.uid.to_be_bytes());
    opaque[31] = 1;
    let caller = CallerContext::from_transport(
        opaque,
        peer.uid,
        TransactionId::from_bytes([1; 16]).unwrap(),
    );
    let connection = crate::server::Connection {
        session,
        peer,
        fd: pair.platform,
    };
    let (actor, grant, _) = unavailable_operation();
    let id = actor.state().common.context.operation_id;
    let clock =
        carosign_adapters::clock::StepClock::new(actor.state().common.context.deadline.boot());
    let registry = OperationRegistry::new();
    let mut guard = registry.reserve(actor, grant, &caller).unwrap();
    mark_fixture_recorded(&mut guard);
    guard
        .operation_mut()
        .events
        .lock()
        .unwrap()
        .publish(
            carosign_engine::OperationPhase::Running,
            EventKind::StateChanged,
        )
        .unwrap();
    drop(guard);
    let prepared = futures_executor::block_on(registry.prepare_owner_verification(
        id,
        &caller,
        &Authority {
            deny: false,
            inject: None,
        },
        &clock,
    ))
    .unwrap();
    let (operation, outcome, sequence) = prepared.into_parts();
    let bytes = crate::answer::terminal_frame(operation, outcome, sequence).encode_to_vec();
    crate::server::write_frame(&connection, &bytes).unwrap();
    let mut buffer = vec![0; 65536];
    // SAFETY: The descriptor and buffer are active during recv.
    let size = unsafe {
        libc::recv(
            pair.worker.as_raw_fd(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            0,
        )
    };
    assert!(size > 0);
    let frame =
        carosign_protocol::carosign::client::v1::ClientFrame::decode(&buffer[..size as usize])
            .unwrap();
    assert_eq!(frame.event_sequence, 2);
    assert!(
        matches!(frame.payload, Some(carosign_protocol::carosign::client::v1::client_frame::Payload::OperationEvent(event)) if event.sequence == 2 && event.terminal.is_some())
    );
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &caller,
            &Authority {
                deny: false,
                inject: None
            },
            &clock
        )),
        Err(OwnerDeliveryFailure::AlreadyReserved)
    ));
    drop(pair.worker);
    assert!(crate::server::write_frame(&connection, &bytes).is_err());
}

#[test]
fn terminal_disconnect_notification_is_retained_until_exact_acknowledgement() {
    use carosign_runtime::operation::{events::EventKind, observer::OBSERVER_QUEUE_MAX};
    let (actor, grant, caller) = unavailable_operation();
    let id = actor.state().common.context.operation_id;
    let clock =
        carosign_adapters::clock::StepClock::new(actor.state().common.context.deadline.boot());
    let registry = OperationRegistry::new();
    let mut guard = registry.reserve(actor, grant, &caller).unwrap();
    mark_fixture_recorded(&mut guard);
    {
        let mut hub = guard.operation_mut().events.lock().unwrap();
        hub.attach(42, 0).unwrap();
        for _ in 0..OBSERVER_QUEUE_MAX {
            assert!(
                hub.publish(
                    carosign_engine::OperationPhase::Running,
                    EventKind::StateChanged
                )
                .unwrap()
                .is_empty()
            );
        }
    }
    drop(guard);
    let prepared = futures_executor::block_on(registry.prepare_owner_verification(
        id,
        &caller,
        &Authority {
            deny: false,
            inject: None,
        },
        &clock,
    ))
    .unwrap();
    assert_eq!(prepared.into_parts().2, OBSERVER_QUEUE_MAX as u64 + 1);
    assert_eq!(registry.terminal_disconnects(id).unwrap(), vec![42]);
    assert!(matches!(
        registry.acknowledge_terminal_disconnects(id, &[41]),
        Err(ObservationFailure::ContextMismatch)
    ));
    assert_eq!(registry.terminal_disconnects(id).unwrap(), vec![42]);
    registry
        .acknowledge_terminal_disconnects(id, &[42])
        .unwrap();
    assert!(registry.terminal_disconnects(id).unwrap().is_empty());
}

#[test]
fn revocation_before_terminal_publication_does_not_advance_stream() {
    let (actor, grant, caller) = unavailable_operation();
    let id = actor.state().common.context.operation_id;
    let registry = OperationRegistry::new();
    let mut guard = registry.reserve(actor, grant, &caller).unwrap();
    assert!(registry.revoke_verification(id).is_err());
    assert!(matches!(
        guard.publish_owner_terminal(),
        Err(OwnerDeliveryFailure::Authority(
            carosign_runtime::ports::AuthorityRefusal::Revoked
        ))
    ));
    assert_eq!(
        guard
            .operation_mut()
            .events
            .lock()
            .unwrap()
            .stream()
            .last_sequence(),
        0
    );
    assert!(!guard.operation_mut().delivery_reserved);
}

#[test]
fn owner_delivery_rechecks_control_after_await_without_holding_registry_lock() {
    let (actor, grant, caller) = unavailable_operation();
    let id = actor.state().common.context.operation_id;
    let clock =
        carosign_adapters::clock::StepClock::new(actor.state().common.context.deadline.boot());
    let registry = OperationRegistry::new();
    drop(registry.reserve(actor, grant, &caller).unwrap());
    let source = Authority {
        deny: false,
        inject: Some((&registry, id)),
    };
    assert!(matches!(
        futures_executor::block_on(
            registry.prepare_owner_verification(id, &caller, &source, &clock)
        ),
        Err(OwnerDeliveryFailure::Operation(
            ObservationFailure::ContextMismatch
        ))
    ));
}

#[test]
fn owner_delivery_rejects_expiry_and_unreleased_lease() {
    let (actor, grant, caller) = unavailable_operation();
    let id = actor.state().common.context.operation_id;
    let deadline = actor.state().common.context.deadline;
    let auth = actor.state().common.context.authenticator;
    let registry = OperationRegistry::new();
    let mut guard = registry.reserve(actor, grant, &caller).unwrap();
    guard.operation_mut().lease = Some(carosign_runtime::ports::ResourceLease::issued_for_fixture(
        carosign_core::ids::LeaseId::from_bytes([8; 16]).unwrap(),
        carosign_runtime::ports::ResourceOwner {
            operation: id,
            worker: carosign_core::ids::WorkerInstanceId::from_bytes([9; 16]).unwrap(),
            authenticator: auth,
        },
        fixture_set(
            vec![carosign_core::ids::ResourceId::from_bytes([10; 16]).unwrap()],
            carosign_core::resource::AccessMode::Exclusive,
        ),
    ));
    drop(guard);
    let clock = carosign_adapters::clock::StepClock::new(deadline.boot());
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &caller,
            &Authority {
                deny: false,
                inject: None
            },
            &clock
        )),
        Err(OwnerDeliveryFailure::Operation(
            ObservationFailure::ContextMismatch
        ))
    ));
    clock.advance(deadline.instant_ns());
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &caller,
            &Authority {
                deny: false,
                inject: None
            },
            &clock
        )),
        Err(OwnerDeliveryFailure::Operation(
            ObservationFailure::DeadlineExpired
        ))
    ));
    assert!(registry.has_retained_lease(id));
}

#[test]
fn owner_delivery_never_treats_unverified_match_as_accepted_factor() {
    let (actor, grant, caller) = unavailable_operation();
    let mut state = actor.state().clone();
    state.common.terminal = Some(carosign_engine::TerminalOutcome::FactorAccepted);
    let id = state.common.context.operation_id;
    let clock = carosign_adapters::clock::StepClock::new(state.common.context.deadline.boot());
    let registry = OperationRegistry::new();
    let mut guard = registry
        .reserve(OperationActor::new(state), grant, &caller)
        .unwrap();
    mark_fixture_recorded(&mut guard);
    drop(guard);
    assert!(matches!(
        futures_executor::block_on(registry.prepare_owner_verification(
            id,
            &caller,
            &Authority {
                deny: false,
                inject: None
            },
            &clock
        )),
        Err(OwnerDeliveryFailure::EvidenceUnavailable)
    ));
}

#[test]
fn ack_rejects_foreign_worker_device_epoch_and_command() {
    let mut state = carosign_engine::tests_support::initial();
    let context = VerificationContext {
        operation: state.common.context.operation_id,
        worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
        command: CommandId::from_bytes([2; 16]).unwrap(),
        authenticator: state.common.context.authenticator.id,
        epoch: state.common.context.authenticator.device_epoch,
    };
    state.common.worker = Some(carosign_engine::transition::WorkerRef {
        instance: context.worker,
        device_epoch: context.epoch,
    });
    state.common.command = Some(carosign_engine::transition::CommandRef {
        id: context.command,
        transition_index: carosign_engine::TransitionIndex::new(1),
    });
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let registry = OperationRegistry::new();
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    drop(
        registry
            .reserve(OperationActor::new(state), grant, &caller)
            .unwrap(),
    );
    for foreign in [
        VerificationContext {
            worker: WorkerInstanceId::from_bytes([9; 16]).unwrap(),
            ..context
        },
        VerificationContext {
            command: CommandId::from_bytes([9; 16]).unwrap(),
            ..context
        },
        VerificationContext {
            authenticator: AuthenticatorId::from_bytes([9; 16]).unwrap(),
            ..context
        },
        VerificationContext {
            epoch: DeviceEpoch::new(2).unwrap(),
            ..context
        },
    ] {
        assert!(matches!(
            registry.apply_verification_ack(&foreign),
            Err(ObservationFailure::ContextMismatch)
        ));
    }
}
