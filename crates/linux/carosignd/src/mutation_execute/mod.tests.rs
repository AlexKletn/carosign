use super::*;
use carosign_core::ids::{AuthenticatorId, BootId, IdempotencyKey};
use carosign_protocol::carosign::common::v1::{SubjectSelector, subject_selector::Selector};
use carosign_runtime::ports::{CallerContext, TransactionId};
use std::sync::Arc;

fn challenge_scope() -> carosign_runtime::enrollment_authorization::EnrollmentAuthorizationScope {
    use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{
        AuthorityId, CommandId, LeaseId, MutationId, OperationId, SubjectId, WorkerInstanceId,
    };
    carosign_runtime::enrollment_authorization::EnrollmentAuthorizationScope {
        operation: OperationId::from_bytes([1; 16]).unwrap(),
        mutation: MutationId::from_bytes([2; 16]).unwrap(),
        challenge_command: CommandId::from_bytes([3; 16]).unwrap(),
        command: CommandId::from_bytes([4; 16]).unwrap(),
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([5; 16]).unwrap(),
            subject: SubjectId::from_bytes([6; 16]).unwrap(),
        },
        uid: 0,
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([7; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        lease: LeaseId::from_bytes([9; 16]).unwrap(),
        boot: BootId::from_bytes([10; 16]).unwrap(),
        recipient: carosign_runtime::enrollment_authorization::EnrollmentRecipientId::from_bytes(
            [11; 16],
        )
        .unwrap(),
    }
}

#[test]
fn challenge_frames_are_bound_to_operation_and_worker() {
    use carosign_protocol::carosign::provider::v1::{
        EnrollmentChallengeReady, provider_frame::Payload,
    };
    let scope = challenge_scope();
    let request = enrollment_challenge_frame(&scope);
    assert_eq!(request.uid, Some(0));
    assert_eq!(request.command_id, scope.challenge_command.as_bytes());
    let Some(Payload::PrepareEnrollmentChallenge(challenge)) = request.payload else {
        panic!("unexpected payload")
    };
    assert_eq!(challenge.mutation_id, scope.mutation.as_bytes());
    let mut reply = carosign_protocol::carosign::provider::v1::ProviderFrame {
        major: carosign_protocol::limits::MAJOR,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: scope.worker.as_bytes().to_vec(),
        frame_seq: 1,
        command_id: scope.challenge_command.as_bytes().to_vec(),
        operation_id: scope.operation.as_bytes().to_vec(),
        device_epoch: scope.authenticator.device_epoch.value(),
        uid: None,
        payload: Some(Payload::EnrollmentChallengeReady(
            EnrollmentChallengeReady {
                mutation_id: scope.mutation.as_bytes().to_vec(),
                challenge: vec![11],
                challenge_ref: vec![12; 16],
                validity_ms: 1000,
            },
        )),
    };
    assert!(enrollment_challenge_observation(&scope, reply.clone()).is_ok());
    reply.command_id = vec![13; 16];
    assert!(matches!(
        enrollment_challenge_observation(&scope, reply.clone()),
        Err(MutationStartError::Context)
    ));
    reply.command_id = scope.challenge_command.as_bytes().to_vec();
    if let Some(Payload::EnrollmentChallengeReady(value)) = reply.payload.as_mut() {
        value.mutation_id = vec![14; 16];
    }
    assert!(matches!(
        enrollment_challenge_observation(&scope, reply),
        Err(MutationStartError::Context)
    ));
}

#[test]
fn challenge_authorization_is_revoked_on_cancel_and_normal_cleanup() {
    use carosign_runtime::enrollment_authorization::{
        EnrollmentAuthorizationAuthority, EnrollmentAuthorizationError,
        EnrollmentAuthorizationScope,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Authority(AtomicUsize);
    impl EnrollmentAuthorizationAuthority for Authority {
        fn revoke_now(
            &self,
            _: &EnrollmentAuthorizationScope,
        ) -> Result<(), EnrollmentAuthorizationError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    let authority = Arc::new(Authority(AtomicUsize::new(0)));
    {
        let _cancelled = EnrollmentAuthorizationGuard {
            authority: authority.clone(),
            scope: challenge_scope(),
            authorization: None,
            admission: None,
            revoked: false,
        };
    }
    assert_eq!(authority.0.load(Ordering::SeqCst), 1);
    {
        let mut completed = EnrollmentAuthorizationGuard {
            authority: authority.clone(),
            scope: challenge_scope(),
            authorization: None,
            admission: None,
            revoked: false,
        };
        completed.revoke().unwrap();
    }
    assert_eq!(authority.0.load(Ordering::SeqCst), 2);
}

#[test]
#[ignore = "требует CAROSIGN_TEST_BROKER и CAROSIGN_TEST_PROVIDER"]
fn actor_arms_and_receives_real_mock_enrollment() {
    let dir = std::env::temp_dir().join(format!("carosign-mutation-start-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let store =
        carosign_persistence::redb::backend::RedbStore::create_current(&dir.join("store.redb"))
            .unwrap();
    let uid = unsafe { libc::getuid() };
    let path = dir.join("policy");
    std::fs::write(
        &path,
        format!("uid={uid} subject={uid} actions=enroll,remove lifetime=30\n"),
    )
    .unwrap();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        BootId::from_bytes(carosign_linux::sys::boot_id().unwrap()).unwrap(),
    ));
    let policy = crate::linux_policy::LinuxPolicy::fixture(&path, clock.clone());
    let caller =
        CallerContext::from_transport([1; 32], uid, TransactionId::from_bytes([2; 16]).unwrap());
    let profile = crate::provider_supervisor::ProviderProfile::fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let request = carosign_linux_ipc::mapping::client::DomainRequest::BeginEnrollment {
        idempotency: IdempotencyKey::from_bytes([3; 16]).unwrap(),
        authenticator: AuthenticatorId::from_bytes([5; 16]).unwrap(),
        label: "test".into(),
        subject: SubjectSelector {
            selector: Some(Selector::CurrentSubject(true)),
        },
        replace: None,
        authority_request_ref: vec![],
    };
    let prepared = futures_executor::block_on(crate::mutation_dispatch::prepare(
        &store,
        &policy,
        &caller,
        &request,
        profile.identity().authenticator,
    ))
    .unwrap();
    let registry = Arc::new(crate::operations::OperationRegistry::default());
    let handle = registry.open_connection(4).unwrap();
    let resources = Arc::new(crate::resources::LocalResourceOwner::default());
    let active = futures_executor::block_on(start_inner(
        &prepared,
        &store,
        &policy,
        &profile,
        &resources,
        &registry,
        handle,
        clock.as_ref(),
        Arc::new(carosign_runtime::enrollment_authorization::UnavailableEnrollmentAuthority),
        None,
    ))
    .unwrap();
    let MutationReception::Observed(observed) = futures_executor::block_on(active.receive(
        &prepared,
        &store,
        &policy,
        &registry,
        &resources,
        |_| Ok(()),
    ))
    .unwrap() else {
        panic!("неожиданный откат")
    };
    assert!(!observed.provider_ref.is_empty());
    assert_eq!(
        registry.phase(observed.active.operation),
        Some(carosign_engine::OperationPhase::ValidatingChange)
    );
    let mutation = observed.active.mutation;
    let operation = futures_executor::block_on(finish(
        observed, &prepared, &store, &policy, &resources, &registry,
    ))
    .unwrap();
    let completed =
        carosign_persistence::mutation_store::read_completed_v4(&store, mutation, operation)
            .unwrap();
    assert_eq!(
        completed.terminal_outcome,
        Some(carosign_persistence::model::MutationTerminalOutcome::Committed)
    );
    assert_eq!(
        crate::accept::read_enrollment_records(
            &store,
            &crate::accept::scope_of(prepared.subject, prepared.authenticator)
        )
        .unwrap()
        .len(),
        1
    );
    if std::env::var("CAROSIGN_TEST_SCENARIO").as_deref() == Ok("linux-persistent") {
        let plan = completed.plan.as_ref().unwrap();
        let removal = carosign_linux_ipc::mapping::client::DomainRequest::BeginRemoval {
            idempotency: IdempotencyKey::from_bytes([4; 16]).unwrap(),
            authenticator: prepared.authenticator,
            enrollment: plan.new_enrollment.unwrap(),
            generation: plan.new_generation,
            subject: SubjectSelector {
                selector: Some(Selector::CurrentSubject(true)),
            },
            authority_request_ref: vec![],
        };
        let prepared = futures_executor::block_on(crate::mutation_dispatch::prepare(
            &store,
            &policy,
            &caller,
            &removal,
            profile.identity().authenticator,
        ))
        .unwrap();
        let active = futures_executor::block_on(start_inner(
            &prepared,
            &store,
            &policy,
            &profile,
            &resources,
            &registry,
            handle,
            clock.as_ref(),
            Arc::new(carosign_runtime::enrollment_authorization::UnavailableEnrollmentAuthority),
            None,
        ))
        .unwrap();
        let MutationReception::Observed(observed) = futures_executor::block_on(active.receive(
            &prepared,
            &store,
            &policy,
            &registry,
            &resources,
            |_| Ok(()),
        ))
        .unwrap() else {
            panic!("неожиданный откат")
        };
        let mutation = observed.active.mutation;
        let operation = futures_executor::block_on(finish(
            observed, &prepared, &store, &policy, &resources, &registry,
        ))
        .unwrap();
        let completed =
            carosign_persistence::mutation_store::read_completed_v4(&store, mutation, operation)
                .unwrap();
        assert_eq!(
            completed.terminal_outcome,
            Some(carosign_persistence::model::MutationTerminalOutcome::Committed)
        );
        assert!(
            crate::accept::read_enrollment_records(
                &store,
                &crate::accept::scope_of(prepared.subject, prepared.authenticator)
            )
            .unwrap()
            .is_empty()
        );
    }
}
