//! Verification of end-to-end service wiring with dummy trusted ports.
//!
//! This stand does not prove trusted launch or hardware peace.

#[path = "service_completion_tests_fixtures/cancel_state.rs"]
mod cancel_state;
#[path = "service_completion_tests_fixtures/cleanup.rs"]
mod cleanup;
#[path = "service_completion_tests_fixtures/client_authority.rs"]
mod client_authority;
#[path = "service_completion_tests_fixtures/client_end.rs"]
mod client_end;
#[path = "service_completion_tests_fixtures/client_run.rs"]
mod client_run;
#[path = "service_completion_tests_fixtures/dispatch_case.rs"]
mod dispatch_case;
#[path = "service_completion_tests_fixtures/dispatcher.rs"]
mod dispatcher;
#[path = "service_completion_tests_fixtures/expiring_resources.rs"]
mod expiring_resources;
#[path = "service_completion_tests_fixtures/foreign_resources.rs"]
mod foreign_resources;
#[path = "service_completion_tests_fixtures/foreign_worker_resources.rs"]
mod foreign_worker_resources;
#[path = "service_completion_tests_fixtures/ids.rs"]
mod ids;
#[path = "service_completion_tests_fixtures/mock_child.rs"]
mod mock_child;
#[path = "service_completion_tests_fixtures/profile.rs"]
mod profile;
#[path = "service_completion_tests_fixtures/resources.rs"]
mod resources;
#[path = "service_completion_tests_fixtures/revoke_during_delivery.rs"]
mod revoke_during_delivery;
#[path = "service_completion_tests_fixtures/revoking_resources.rs"]
mod revoking_resources;
#[path = "service_completion_tests_fixtures/scheduled_resources.rs"]
mod scheduled_resources;
#[path = "service_completion_tests_fixtures/scoped_provider.rs"]
mod scoped_provider;
#[path = "service_completion_tests_fixtures/scoped_verifier.rs"]
mod scoped_verifier;
#[path = "service_completion_tests_fixtures/shared_cleanup.rs"]
mod shared_cleanup;
#[path = "service_completion_tests_fixtures/shared_profile.rs"]
mod shared_profile;
#[path = "service_completion_tests_fixtures/test_bridge.rs"]
mod test_bridge;
#[path = "service_completion_tests_fixtures/test_verifier.rs"]
mod test_verifier;
#[path = "service_completion_tests_fixtures/uncertain_cleanup.rs"]
mod uncertain_cleanup;
#[path = "service_completion_tests_fixtures/unused_journal.rs"]
mod unused_journal;
#[path = "service_completion_tests_fixtures/unused_provider.rs"]
mod unused_provider;

use carosign_core::authorization::{AuthorizationBoundary, GrantScope, ValidatedGrant};
use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::*;
use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
use carosign_persistence::mutation_plan::StoredBindingKind;
use carosign_persistence::store::{MetadataStore, Table, tests_support::MemoryStore};
use carosign_runtime::ports::*;

use ids::Ids;

use client_authority::ClientAuthority;

#[test]
fn counted_conversation_serves_real_socket_and_releases_slot() {
    use crate::talk::{Asked, CountedServeError, Ended};
    use carosign_protocol::carosign::client::v1::{self as client, client_frame::Payload};
    use carosign_protocol::carosign::common::v1::ErrorCode;

    let path = std::env::temp_dir().join(format!(
        "carosign-counted-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = path.to_str().unwrap().to_owned();
    let listener = carosign_linux::sys::listen_unix(&path).unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([4; 16]).unwrap(),
        subject: SubjectId::from_bytes([5; 16]).unwrap(),
    };
    let boot = BootId::from_bytes([21; 16]).unwrap();
    let authority = ClientAuthority {
        caller: CallerContext::from_transport(
            [9; 32],
            1000,
            TransactionId::from_bytes([20; 16]).unwrap(),
        ),
        subject,
        boot,
        refusal: None,
    };
    let operations = std::sync::Arc::new(crate::operations::OperationRegistry::new());
    let store = MemoryStore::default();
    let clock = carosign_adapters::clock::StepClock::new(boot);
    let service = crate::service::Service {
        store: &store,
        authority: &authority,
        ids: &Ids,
        clock: &clock,
        resources: &Resources,
        resource_sets: &Profile,
        operations: &operations,
        provider: &UnusedProvider,
        mutation: &UnusedJournal,
    };
    let asked = Asked {
        subject,
        epoch: DeviceEpoch::FIRST,
    };
    let health = crate::health::UnavailableHealth;
    let mut config = crate::config::Config {
        hello_timeout_s: 0,
        ..Default::default()
    };
    assert!(matches!(
        futures_executor::block_on(crate::talk::serve_counted_once(
            &listener, &service, asked, &health, &config,
        )),
        Err(CountedServeError::InvalidConfig)
    ));
    config.hello_timeout_s = 1;
    let mut held = Vec::new();
    for _ in 1..crate::operations::DEFAULT_CONNECTIONS {
        held.push(operations.open_connection(4).unwrap());
    }
    let client_registry = operations.clone();
    let client_path = path.clone();
    let client = std::thread::spawn(move || {
        let transport =
            carosign_linux_ipc::client::transport::Transport::connect(&client_path).unwrap();
        assert!(client_registry.open_connection(4).is_none());
        for payload in [
            Payload::GetHealth(client::GetHealth {}),
            Payload::ListAuthenticators(client::ListAuthenticators {}),
        ] {
            let answer = transport.request(payload).unwrap();
            assert!(
                matches!(answer.payload, Some(Payload::ProtocolError(error)) if error.error.as_ref().is_some_and(|error| error.code == ErrorCode::ErrorUnavailable as i32))
            );
        }
    });
    assert!(matches!(
        futures_executor::block_on(crate::talk::serve_counted_once(
            &listener, &service, asked, &health, &config,
        )),
        Ok(Ended::Closed)
    ));
    client.join().unwrap();
    let restored = operations
        .open_connection(4)
        .expect("сеанс освободил место");
    operations.close_connection(restored);
    for handle in held {
        operations.close_connection(handle);
    }
    drop(listener);
    std::fs::remove_file(path).unwrap();
}

use revoke_during_delivery::RevokeDuringDelivery;

#[test]
fn configured_hello_deadline_closes_silent_real_connection() {
    use crate::talk::{Asked, Ended};
    use std::time::{Duration, Instant};

    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = carosign_linux::sys::peer_credentials(&pair.platform).unwrap();
    let credentials =
        carosign_linux_ipc::credentials::PeerCredentials::from_kernel(peer.pid, peer.uid, peer.gid)
            .unwrap();
    let mut connection = crate::server::Connection {
        session: carosign_linux_ipc::client::session::ClientSession::new(credentials, [20; 16]),
        peer,
        fd: pair.platform,
    };
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([4; 16]).unwrap(),
        subject: SubjectId::from_bytes([5; 16]).unwrap(),
    };
    let boot = BootId::from_bytes([21; 16]).unwrap();
    let authority = ClientAuthority {
        caller: CallerContext::from_transport(
            [9; 32],
            peer.uid,
            TransactionId::from_bytes([20; 16]).unwrap(),
        ),
        subject,
        boot,
        refusal: None,
    };
    let store = MemoryStore::default();
    let operations = crate::operations::OperationRegistry::new();
    let clock = carosign_adapters::clock::StepClock::new(boot);
    let service = crate::service::Service {
        store: &store,
        authority: &authority,
        ids: &Ids,
        clock: &clock,
        resources: &Resources,
        resource_sets: &Profile,
        operations: &operations,
        provider: &UnusedProvider,
        mutation: &UnusedJournal,
    };
    let mut config = crate::config::Config {
        hello_timeout_s: 0,
        ..Default::default()
    };
    let asked = Asked {
        subject,
        epoch: DeviceEpoch::FIRST,
    };
    assert!(matches!(
        futures_executor::block_on(crate::talk::converse_configured(
            &mut connection, &service, asked, &crate::health::UnavailableHealth, &config,
        )),
        Ended::Refused(reason) if reason == "invalid configuration"
    ));
    config.hello_timeout_s = 1;
    let start = Instant::now();
    assert!(matches!(
        futures_executor::block_on(crate::talk::converse_configured(
            &mut connection, &service, asked, &crate::health::UnavailableHealth, &config,
        )),
        Ended::Refused(reason) if reason == "hello timeout"
    ));
    assert!(start.elapsed() >= Duration::from_secs(1));
    assert!(start.elapsed() < Duration::from_secs(4));
    drop(pair.worker);
}

#[test]
fn client_preparation_keeps_policy_purpose_and_request_context() {
    use carosign_linux_ipc::mapping::client::to_domain;
    use carosign_protocol::carosign::client::v1::{self as client, client_frame::Payload};
    use carosign_protocol::carosign::common::v1::{
        SubjectSelector as WireSelector, VerificationPurpose, subject_selector::Selector,
    };
    use prost::Message;
    use std::os::fd::AsRawFd;
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([4; 16]).unwrap(),
        subject: SubjectId::from_bytes([5; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([7; 16]).unwrap();
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([6; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let record = EnrollmentRecord {
        subject,
        enrollment,
        display_label: None,
        label: "тест".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![8]).unwrap(),
        format_id: "test-format".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        enrollment.id.as_bytes(),
        &carosign_persistence::enrollment_codec::encode_v3(&record).unwrap(),
    )
    .unwrap();
    let mut key = crate::accept::scope_of(subject, authenticator).to_vec();
    key.extend_from_slice(enrollment.id.as_bytes());
    tx.put(Table::EnrollmentIndex, &key, enrollment.id.as_bytes())
        .unwrap();
    tx.commit_durable().unwrap();
    let socket = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = carosign_linux::sys::peer_credentials(&socket.platform).unwrap();
    let credentials =
        carosign_linux_ipc::credentials::PeerCredentials::from_kernel(peer.pid, peer.uid, peer.gid)
            .unwrap();
    let mut connection = crate::server::Connection {
        session: carosign_linux_ipc::client::session::ClientSession::new(credentials, [20; 16]),
        peer,
        fd: socket.platform,
    };
    let send_frame = |frame: client::ClientFrame| {
        let bytes = frame.encode_to_vec();
        // SAFETY: The socket and buffer remain valid throughout the call.
        let sent = unsafe {
            libc::send(
                socket.worker.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                0,
            )
        };
        assert_eq!(sent, bytes.len() as isize);
    };
    let mut buffer = vec![0; carosign_protocol::limits::FRAME_MAX];
    send_frame(client::ClientFrame {
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
    });
    let len = crate::server::read_frame(&connection, &mut buffer)
        .unwrap()
        .unwrap();
    crate::server::greet(&mut connection, &buffer[..len]).unwrap();
    let caller = CallerContext::from_transport(
        [9; 32],
        peer.uid,
        TransactionId::from_bytes([20; 16]).unwrap(),
    );
    let boot = BootId::from_bytes([21; 16]).unwrap();
    let clock = carosign_adapters::clock::StepClock::new(boot);
    let operations = crate::operations::OperationRegistry::new();
    let authority = ClientAuthority {
        caller: caller.clone(),
        subject,
        boot,
        refusal: None,
    };
    let service = crate::service::Service {
        store: &store,
        authority: &authority,
        ids: &Ids,
        clock: &clock,
        resources: &Resources,
        resource_sets: &Profile,
        operations: &operations,
        provider: &UnusedProvider,
        mutation: &UnusedJournal,
    };
    for (index, selector) in [
        Selector::CurrentSubject(true),
        Selector::AccountHint("имя".into()),
        Selector::OpaqueAuthoritySelector(vec![1, 2, 3]),
    ]
    .into_iter()
    .enumerate()
    {
        send_frame(client::ClientFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_MAX,
            request_id: vec![2; 16],
            operation_id: Vec::new(),
            event_sequence: 0,
            payload: Some(Payload::BeginVerification(client::BeginVerification {
                report_matched_enrollment: false,
                enrollment: None,
                idempotency_key: vec![12; 16],
                authenticator_id: authenticator.as_bytes().to_vec(),
                subject: Some(WireSelector {
                    selector: Some(selector),
                }),
                purpose_hint: VerificationPurpose::PurposeSessionLogin as i32,
                authority_request_ref: vec![9, 8],
            })),
        });
        let len = crate::server::read_frame(&connection, &mut buffer)
            .unwrap()
            .unwrap();
        let decoded = connection.session.accept(&buffer[..len]).unwrap();
        let request = to_domain(&decoded).unwrap().unwrap();
        let prepared = futures_executor::block_on(service.prepare_client_verification(
            &caller,
            &request,
            &authority,
            DeviceEpoch::FIRST,
        ))
        .unwrap();
        assert!(prepared.caller == caller);
        assert_eq!(prepared.idempotency.as_bytes(), &[12; 16]);
        assert_eq!(prepared.intent.authority_request_ref, [9, 8]);
        assert!(matches!(
            prepared.cycle.scope,
            GrantScope::Verify {
                purpose: carosign_core::operation::context::VerificationPurpose::SessionUnlock,
                ..
            }
        ));
        assert!(prepared.cycle.scope == *prepared.grant.scope());
        if index == 0 {
            use crate::operations::{IdempotentAdmission, TrustedCallerBinding};
            let connection_handle = operations.open_connection(4).unwrap();
            let binding = TrustedCallerBinding::fixture([42; 32]);
            let admit = |prepared: &crate::service::PreparedClientVerification| {
                service.admit_client_verification(
                    prepared,
                    DeviceEpoch::FIRST,
                    Some(&binding),
                    connection_handle,
                    8,
                )
            };
            assert_eq!(
                service.admit_client_verification(
                    &prepared,
                    DeviceEpoch::FIRST,
                    None,
                    connection_handle,
                    8
                ),
                IdempotentAdmission::Unavailable
            );
            let mut forged = crate::service::PreparedClientVerification {
                caller: prepared.caller.clone(),
                idempotency: prepared.idempotency,
                intent: prepared.intent.clone(),
                cycle: prepared.cycle.clone(),
                grant: prepared.grant.clone(),
            };
            forged.cycle.caller = CallerContext::from_transport(
                [77; 32],
                peer.uid,
                TransactionId::from_bytes([20; 16]).unwrap(),
            );
            assert_eq!(admit(&forged), IdempotentAdmission::Unavailable);
            forged.cycle = prepared.cycle.clone();
            forged.intent.authenticator = AuthenticatorId::from_bytes([77; 16]).unwrap();
            assert_eq!(admit(&forged), IdempotentAdmission::Unavailable);
            let IdempotentAdmission::Created(operation) = admit(&prepared) else {
                panic!("операция не опубликована");
            };
            {
                let mut guard = operations.checkout(operation).unwrap();
                let saved = guard.operation_mut();
                assert!(saved.fixture_owner() == &caller);
                assert!(saved.actor.state().required_grant_scope() == *prepared.grant.scope());
                assert!(saved.actor.has_work());
            }
            let again = futures_executor::block_on(service.prepare_client_verification(
                &caller,
                &request,
                &authority,
                DeviceEpoch::FIRST,
            ))
            .unwrap();
            assert_eq!(admit(&again), IdempotentAdmission::Existing(operation));
            {
                let mut guard = operations.checkout(operation).unwrap();
                let actor = &mut guard.operation_mut().actor;
                assert!(matches!(
                    actor.run_once(),
                    carosign_runtime::operation::actor::Step::Advanced { state, .. }
                        if state.common.phase == carosign_engine::OperationPhase::WaitingLease
                ));
                assert!(!actor.has_work());
            }
            let mut changed_request = request.clone();
            let carosign_linux_ipc::mapping::client::DomainRequest::BeginVerification {
                authority_request_ref,
                ..
            } = &mut changed_request
            else {
                unreachable!()
            };
            authority_request_ref.push(1);
            let changed = futures_executor::block_on(service.prepare_client_verification(
                &caller,
                &changed_request,
                &authority,
                DeviceEpoch::FIRST,
            ))
            .unwrap();
            assert_eq!(admit(&changed), IdempotentAdmission::Conflict);
        }
        let forged = CallerContext::from_transport(
            [10; 32],
            peer.uid,
            TransactionId::from_bytes([20; 16]).unwrap(),
        );
        assert!(
            futures_executor::block_on(service.prepare_client_verification(
                &forged,
                &request,
                &authority,
                DeviceEpoch::FIRST
            ))
            .is_err()
        );
        let revoked = ClientAuthority {
            caller: caller.clone(),
            subject,
            boot,
            refusal: Some(AuthorityRefusal::Revoked),
        };
        let revoked_service = crate::service::Service {
            authority: &revoked,
            ..service
        };
        assert!(matches!(
            futures_executor::block_on(revoked_service.prepare_client_verification(
                &caller,
                &request,
                &revoked,
                DeviceEpoch::FIRST,
            )),
            Err(crate::service::ClientPreparationError::Refused(
                crate::accept::AcceptError::Unauthorized(AuthorityRefusal::Revoked)
            ))
        ));
    }
}
use profile::Profile;
use resources::Resources;
use shared_profile::SharedProfile;

use scheduled_resources::ScheduledResources;

use foreign_resources::ForeignResources;

use foreign_worker_resources::ForeignWorkerResources;

use revoking_resources::RevokingResources;

use expiring_resources::ExpiringResources;

use unused_provider::UnusedProvider;

use unused_journal::UnusedJournal;

#[test]
fn scoped_cancel_uses_held_lease_and_is_not_repeated_after_unknown_result() {
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner, Execution};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use scoped_provider::ScopedProvider;

    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let lease = ResourceLease::issued_for_fixture(
        LeaseId::from_bytes([10; 16]).unwrap(),
        ResourceOwner {
            operation,
            worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
            authenticator: AuthenticatorRef {
                id: AuthenticatorId::from_bytes([7; 16]).unwrap(),
                device_epoch: DeviceEpoch::FIRST,
            },
        },
        ResourceSet::new(vec![ResourceClaim {
            id: ResourceId::from_bytes([11; 16]).unwrap(),
            mode: AccessMode::Exclusive,
        }])
        .unwrap(),
    );
    let provider = ScopedProvider(AtomicUsize::new(0));
    let journal = UnusedJournal;
    let ids = Ids;
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default())
        .with_scoped_cancel(&lease, &ids);
    let effect = PlannedEffect::at(EffectSpec::RequestCancel, TransitionIndex::new(1), 0);
    assert_eq!(
        futures_executor::block_on(runner.execute(effect, operation)),
        Execution::CancelUnconfirmed(DispatchOutcome::Unknown)
    );
    assert_eq!(
        futures_executor::block_on(runner.execute(effect, operation)),
        Execution::Noop
    );
    assert_eq!(provider.0.load(Ordering::SeqCst), 1);
}

use cleanup::Cleanup;

use shared_cleanup::SharedCleanup;

use mock_child::MockChild;

#[test]
fn mock_child_timeout_kills_and_reaps_process() {
    let child = std::process::Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let pid = child.id() as libc::pid_t;
    let mut child = MockChild::new(child).unwrap();
    assert!(!child.has_exited().unwrap());
    assert_eq!(
        child
            .wait_bounded(std::time::Duration::from_millis(20))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::TimedOut
    );
    drop(child);
    let mut status = 0;
    // SAFETY: pid is derived from a child process, status indicates the whole.
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
fn mock_child_observes_and_reaps_exit_with_pidfd() {
    let child = std::process::Command::new("/bin/true").spawn().unwrap();
    let mut child = MockChild::new(child).unwrap();
    assert!(
        child
            .wait_bounded(std::time::Duration::from_secs(2))
            .unwrap()
            .success()
    );
    assert!(child.has_exited().unwrap());
    assert_eq!(
        child.require_alive().unwrap_err().kind(),
        std::io::ErrorKind::BrokenPipe
    );
}

/// Runs a real test process and manually runs its protocol until it's ready, and it's not
/// testing a trusted launch or a proxy.
fn prepare_mock_process(
    binary: &std::path::Path,
    sockets: carosign_linux::sys::SocketPair,
) -> (
    carosign_linux_ipc::provider::transport::ProviderTransport,
    MockChild,
) {
    prepare_mock_process_scenario(binary, sockets, "no-sample")
}

/// Test Tunnel: Trust and an empty set of resources belong to the fixture, and the
/// broker and mock are executed by separate processes.
fn prepare_tunnel_mock_process(
    binary: &std::path::Path,
    sockets: carosign_linux::sys::SocketPair,
    scenario: &str,
) -> (
    crate::provider_verification::VerificationTransport,
    MockChild,
    MockChild,
) {
    use carosign_linux_ipc::provider::tunnel::TunnelClient;
    use carosign_protocol::carosign::broker_relay::v1::{Claim, Owner, Scope};
    use carosign_protocol::carosign::common::v1::AuthenticatorRef;
    use carosign_protocol::carosign::provider::v1::{self as provider, provider_frame::Payload};
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::time::Duration;

    let tunnel = carosign_linux::sys::socketpair_seqpacket().unwrap();
    for fd in [tunnel.worker.as_raw_fd(), sockets.platform.as_raw_fd()] {
        // SAFETY: Both descriptors belong to the test before the mediator is launched.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
            0
        );
    }
    let runner = std::env::var_os("CAROSIGN_RELAY_TEST_RUNNER").expect("путь к тестовому broker");
    let broker = std::process::Command::new(runner)
        .arg(tunnel.worker.as_raw_fd().to_string())
        .arg(sockets.platform.as_raw_fd().to_string())
        .spawn()
        .unwrap();
    drop(tunnel.worker);
    drop(sockets.platform);
    let worker_fd = sockets.worker.as_raw_fd();
    let mut worker = std::process::Command::new(binary);
    worker.args(["198", scenario]);
    // SAFETY: After fork, only async-signal-safe dup2 is performed.
    unsafe {
        worker.pre_exec(move || {
            if libc::dup2(worker_fd, 198) < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let worker = worker.spawn().unwrap();
    drop(sockets.worker);
    let timeout = Duration::from_secs(10);
    let mut client =
        TunnelClient::connect(tunnel.platform, [1; 16], 7, [3; 16], [4; 16], timeout).unwrap();
    assert!(matches!(
        client.receive_event(timeout).unwrap().payload,
        Some(Payload::WorkerHello(_))
    ));
    let frame = |epoch, command: Vec<u8>, operation: Vec<u8>, payload| provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: command,
        operation_id: operation,
        device_epoch: epoch,
        uid: matches!(
            &payload,
            Payload::StartVerification(_)
                | Payload::PrepareEnrollmentChallenge(_)
                | Payload::StartEnrollment(_)
                | Payload::StartRemoval(_)
        )
        .then_some(1000),
        payload: Some(payload),
    };
    client
        .command(
            frame(
                0,
                vec![],
                vec![],
                Payload::Initialize(provider::Initialize {
                    worker_instance_id: vec![1; 16],
                    selected_minor: carosign_protocol::limits::MINOR_MAX,
                    package_id: vec![2; 16],
                    profile_id: vec![3; 16],
                    enabled_features: vec![provider::Feature::BasicOperations as i32],
                }),
            ),
            timeout,
        )
        .unwrap();
    assert!(matches!(
        client.receive_event(timeout).unwrap().payload,
        Some(Payload::Initialized(_))
    ));
    let authenticator = AuthenticatorRef {
        authenticator_id: vec![5; 16],
        device_epoch: 7,
    };
    client
        .bind(
            Scope {
                lease_id: vec![6; 16],
                owner: Some(Owner {
                    operation_id: vec![8; 16],
                    worker_instance: vec![1; 16],
                    authenticator_id: vec![5; 16],
                    device_epoch: 7,
                }),
                claims: vec![Claim {
                    resource_id: vec![9; 16],
                    mode: 1,
                }],
            },
            vec![],
            frame(
                7,
                vec![10; 16],
                vec![8; 16],
                Payload::BindAuthenticator(provider::BindAuthenticator {
                    authenticator: Some(authenticator.clone()),
                    profile_id: vec![3; 16],
                    lease_id: vec![6; 16],
                    fd_slots: vec![],
                }),
            ),
            timeout,
        )
        .unwrap();
    assert!(
        matches!(client.receive_event(timeout).unwrap().payload, Some(Payload::AuthenticatorReady(ref ready)) if ready.authenticator.as_ref() == Some(&authenticator))
    );
    (
        crate::provider_verification::VerificationTransport::Tunnel(Box::new(client)),
        MockChild::new(worker).unwrap(),
        MockChild::new(broker).unwrap(),
    )
}

fn prepare_mock_process_scenario(
    binary: &std::path::Path,
    sockets: carosign_linux::sys::SocketPair,
    scenario: &str,
) -> (
    carosign_linux_ipc::provider::transport::ProviderTransport,
    MockChild,
) {
    use carosign_linux_ipc::provider::transport::ProviderTransport;
    use carosign_protocol::carosign::common::v1 as common;
    use carosign_protocol::carosign::provider::v1 as provider;
    use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
    use carosign_provider_sdk::wire::Channel;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;

    const CHILD_FD: i32 = 198;
    let worker_fd = sockets.worker.as_raw_fd();
    // An optional test helper is an executable carosign-linux test file.
    // It applies test insulation and then triggers the same mock ELF.
    // This does not confirm the provenance or cgroup of a full-time supervisor.
    let mut command = if let Some(helper) =
        std::env::var_os("CAROSIGN_SANDBOX_MOCK_HELPER").filter(|_| scenario == "no-sample")
    {
        let mut command = std::process::Command::new(helper);
        command
            .args([
                "--exact",
                "process::root_switch::tests::confined_mock_performs_handshake",
                "--nocapture",
            ])
            .env("CAROSIGN_SANDBOX_MOCK_CHILD", "1")
            .env("CAROSIGN_SANDBOX_MOCK_BINARY", binary)
            .env("CAROSIGN_SANDBOX_MOCK_SOCKET", CHILD_FD.to_string());
        command
    } else {
        let mut command = std::process::Command::new(binary);
        command.arg(CHILD_FD.to_string()).arg(scenario);
        command
    };
    // SAFETY: Only async-signal-safe dup2 is performed between fork and exec.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(worker_fd, CHILD_FD) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = MockChild::new(command.spawn().expect("запуск mock-процесса"))
        .expect("наблюдение mock-процесса через pidfd");
    drop(sockets.worker);
    let mut transport = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
    let timeout = std::time::Duration::from_secs(5);
    let hello = transport.receive(timeout).unwrap();
    assert!(
        matches!(hello.payload, Some(Payload::WorkerHello(ref value)) if value.min_minor == carosign_protocol::limits::MINOR_BASELINE && value.max_minor == carosign_protocol::limits::MINOR_MAX)
    );
    child.require_alive().expect("mock завершился после Hello");
    let frame = |epoch, command: Vec<u8>, operation: Vec<u8>, payload| provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: command,
        operation_id: operation,
        device_epoch: epoch,
        uid: matches!(
            &payload,
            Payload::StartVerification(_)
                | Payload::PrepareEnrollmentChallenge(_)
                | Payload::StartEnrollment(_)
                | Payload::StartRemoval(_)
        )
        .then_some(1000),
        payload: Some(payload),
    };
    transport
        .send(
            frame(
                0,
                vec![],
                vec![],
                Payload::Initialize(provider::Initialize {
                    worker_instance_id: vec![1; 16],
                    selected_minor: carosign_protocol::limits::MINOR_MAX,
                    package_id: vec![2; 16],
                    profile_id: vec![3; 16],
                    enabled_features: vec![provider::Feature::BasicOperations as i32],
                }),
            ),
            timeout,
        )
        .unwrap();
    assert!(
        matches!(transport.receive(timeout).unwrap().payload, Some(Payload::Initialized(ref value)) if value.ready_for_binding)
    );
    child
        .require_alive()
        .expect("mock завершился после Initialize");
    // A separate seeded profile coincides with a test tunnel broker.
    let seeded = scenario == "seeded-matched-token";
    let authenticator = common::AuthenticatorRef {
        authenticator_id: vec![if seeded { 5 } else { 7 }; 16],
        device_epoch: if seeded { 7 } else { 1 },
    };
    transport
        .send(
            frame(
                authenticator.device_epoch,
                vec![40; 16],
                vec![],
                Payload::BindAuthenticator(provider::BindAuthenticator {
                    authenticator: Some(authenticator.clone()),
                    profile_id: vec![3; 16],
                    lease_id: vec![7; 16],
                    fd_slots: vec![],
                }),
            ),
            timeout,
        )
        .unwrap();
    assert!(
        matches!(transport.receive(timeout).unwrap().payload, Some(Payload::AuthenticatorReady(ref value)) if value.authenticator.as_ref() == Some(&authenticator))
    );
    child
        .require_alive()
        .expect("mock завершился после BindAuthenticator");
    if seeded {
        // Registration in advance is only in the test mock memory.
        return (transport, child);
    }
    transport
        .send(
            frame(
                1,
                vec![41; 16],
                vec![42; 16],
                Payload::StartEnrollment(provider::StartEnrollment {
                    authenticator: Some(authenticator.clone()),
                    subject: Some(common::SubjectRef {
                        authority_id: vec![4; 16],
                        subject_id: vec![5; 16],
                    }),
                    mutation_id: vec![43; 16],
                    label: "test".into(),
                    replace: None,
                    replace_binding: None,
                    enrollment_authorization_ref: vec![],
                    enrollment_challenge_digest: vec![],
                    planned_enrollment: Some(common::EnrollmentRef {
                        enrollment_id: vec![6; 16],
                        authenticator_id: authenticator.authenticator_id,
                        generation: 1,
                    }),
                }),
            ),
            timeout,
        )
        .unwrap();
    assert!(
        matches!(transport.receive(timeout).unwrap().payload, Some(Payload::CommandAck(ref value)) if value.accepted)
    );
    assert!(
        matches!(transport.receive(timeout).unwrap().payload, Some(Payload::MutationOutcome(ref value)) if value.provider_opaque_ref == vec![6; 16])
    );
    child.require_alive().expect("состояние mock-процесса");
    (transport, child)
}

fn prepare_flow_provider(
    sockets: carosign_linux::sys::SocketPair,
    timeout_result: bool,
) -> (
    carosign_linux_ipc::provider::transport::ProviderTransport,
    Option<std::thread::JoinHandle<()>>,
    Option<MockChild>,
) {
    use carosign_linux_ipc::provider::transport::ProviderTransport;
    use carosign_protocol::carosign::provider::v1::{self as provider, provider_frame::Payload};
    use carosign_provider_sdk::wire::Channel;
    use std::time::Duration;
    if let Some(binary) = std::env::var_os("CAROSIGN_MOCK_PROVIDER_TEST_BINARY") {
        let (transport, child) = prepare_mock_process(std::path::Path::new(&binary), sockets);
        return (transport, None, Some(child));
    }
    assert!(std::env::var_os("CAROSIGN_SANDBOX_MOCK_HELPER").is_none());
    let worker = std::thread::spawn(move || {
        let mut transport = ProviderTransport::new(sockets.worker, Channel::new()).unwrap();
        let request = transport.receive(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            request.payload,
            Some(Payload::StartVerification(_))
        ));
        let response = |payload| provider::ProviderFrame {
            major: request.major,
            minor: request.minor,
            worker_instance_id: request.worker_instance_id.clone(),
            frame_seq: 0,
            command_id: request.command_id.clone(),
            operation_id: request.operation_id.clone(),
            device_epoch: request.device_epoch,
            uid: None,
            payload: Some(payload),
        };
        transport
            .send(
                response(Payload::CommandAck(provider::CommandAck {
                    accepted: true,
                    rejection: None,
                })),
                Duration::from_secs(2),
            )
            .unwrap();
        transport
            .send(
                response(Payload::VerificationOutcome(
                    provider::VerificationOutcome {
                        status: if timeout_result {
                            provider::VerificationStatus::VerifyFailed as i32
                        } else {
                            provider::VerificationStatus::VerifyNoUsableSample as i32
                        },
                        error: timeout_result.then_some(
                            carosign_protocol::carosign::common::v1::Error {
                                code:
                                    carosign_protocol::carosign::common::v1::ErrorCode::ErrorTimeout
                                        as i32,
                                safe_message: "Время проверки истекло".into(),
                            },
                        ),
                        ..Default::default()
                    },
                )),
                Duration::from_secs(2),
            )
            .unwrap();
    });
    (
        ProviderTransport::new(sockets.platform, Channel::new()).unwrap(),
        Some(worker),
        None,
    )
}

use dispatch_case::DispatchCase;

#[allow(clippy::needless_borrow)]
fn exercise_service_cleanup(uncertain: bool, revoke: bool, expire: bool, dispatch: DispatchCase) {
    use crate::effect_owner::EffectOwner;
    use crate::operations::OperationRegistry;
    use crate::provider_verification::{VerificationContext, VerificationPump};
    use crate::verification_owner::{
        AdvanceOutcome, OwnerError, OwnerEvent, VerificationOwner, VerificationStart,
    };
    use carosign_adapters::provider::VerificationFrameContext;
    use carosign_engine::{OperationPhase, TerminalOutcome};
    use carosign_linux_ipc::provider::transport::ProviderTransport;
    use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
    use carosign_protocol::carosign::{client::v1 as client, provider::v1 as provider};
    use carosign_provider_sdk::wire::Channel;
    use carosign_runtime::operation::actor::Step;
    use prost::Message;
    use std::os::fd::AsRawFd;
    use std::time::Duration;

    let assert_no_packet = |socket: &std::os::fd::OwnedFd| {
        let mut byte = [0u8; 1];
        // SAFETY: The socket and buffer remain valid for the nonblocking read.
        let received = unsafe {
            libc::recv(
                socket.as_raw_fd(),
                byte.as_mut_ptr().cast(),
                1,
                libc::MSG_DONTWAIT,
            )
        };
        // Another test may still hold a duplicate end after the fork.
        assert!(
            received == 0
                || (received == -1
                    && std::io::Error::last_os_error().raw_os_error() == Some(libc::EAGAIN))
        );
    };

    let tunnel_mode = matches!(
        dispatch,
        DispatchCase::Normal | DispatchCase::FlowMatched | DispatchCase::FlowScoped
    ) && std::env::var_os("CAROSIGN_FLOW_TEST_TUNNEL").is_some();
    let seeded = tunnel_mode
        || (matches!(dispatch, DispatchCase::FlowMatched)
            && std::env::var_os("CAROSIGN_FLOW_SEEDED_PROFILE").is_some());
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([if seeded { 11 } else { 4 }; 16]).unwrap(),
        subject: SubjectId::from_bytes([if seeded { 12 } else { 5 }; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([if seeded { 5 } else { 7 }; 16]).unwrap();
    let epoch = if seeded {
        DeviceEpoch::new(7).unwrap()
    } else {
        DeviceEpoch::FIRST
    };
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([if seeded { 15 } else { 6 }; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let record = EnrollmentRecord {
        subject,
        enrollment,
        display_label: None,
        label: "тест".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![if seeded { 21 } else { 6 }; 16]).unwrap(),
        format_id: String::new(),
        format_version: 0,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let store = MemoryStore::default();
    let mut transaction = store.begin_write().unwrap();
    transaction
        .put(
            Table::Enrollments,
            enrollment.id.as_bytes(),
            &carosign_persistence::enrollment_codec::encode_v3(&record).unwrap(),
        )
        .unwrap();
    let mut key = crate::accept::scope_of(subject, authenticator).to_vec();
    key.extend_from_slice(enrollment.id.as_bytes());
    transaction
        .put(Table::EnrollmentIndex, &key, enrollment.id.as_bytes())
        .unwrap();
    transaction.commit_durable().unwrap();

    // Real listener and accounted accept: The connection and its place are created in the
    // same way that the service composition uses.
    let operations = std::sync::Arc::new(OperationRegistry::new());
    let socket_path = std::env::temp_dir().join(format!(
        "bio-flow-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = socket_path.to_str().unwrap();
    let listener = carosign_linux::sys::listen_unix(path).unwrap();
    let worker =
        (!matches!(dispatch, DispatchCase::PublicLoop)).then(|| connect_public_client(path));
    let mut counted = if matches!(dispatch, DispatchCase::PublicLoop) {
        None
    } else {
        Some(crate::server::accept_counted(&listener, &operations, 4, &Ids).unwrap())
    };
    use client_end::ClientEnd;
    let uid = counted
        .as_ref()
        .map(|connection| connection.connection.peer.uid)
        // SAFETY: Getuid has no preconditions.
        .unwrap_or_else(|| unsafe { libc::getuid() });
    let connection_id = counted
        .as_ref()
        .map(|connection| connection.connection.session.connection())
        .unwrap_or_else(|| *Ids.new_connection_id().unwrap().as_bytes());
    let caller = CallerContext::from_transport(
        [9; 32],
        uid,
        TransactionId::from_bytes(connection_id).unwrap(),
    );
    let boot = BootId::from_bytes([21; 16]).unwrap();
    let clock = std::sync::Arc::new(carosign_adapters::clock::StepClock::new(boot));
    let authority = ClientAuthority {
        caller: caller.clone(),
        subject,
        boot,
        refusal: None,
    };
    let scheduler = std::sync::Arc::new(std::sync::Mutex::new(
        carosign_runtime::scheduler::Scheduler::new(),
    ));
    scheduler.lock().unwrap().set_epoch(epoch);
    let scheduled_resources = ScheduledResources {
        scheduler: scheduler.clone(),
        clock: clock.clone(),
    };
    let revoking_resources = RevokingResources {
        operations: &operations,
    };
    let expiring_resources = ExpiringResources { clock: &clock };
    let service = crate::service::Service {
        store: &store,
        authority: &authority,
        ids: &Ids,
        clock: clock.as_ref(),
        resources: if matches!(dispatch, DispatchCase::ForeignLease) {
            &ForeignResources
        } else if matches!(dispatch, DispatchCase::ForeignWorker) {
            &ForeignWorkerResources
        } else if matches!(dispatch, DispatchCase::RevokeDuringAcquire) {
            &revoking_resources
        } else if matches!(dispatch, DispatchCase::ExpireDuringAcquire) {
            &expiring_resources
        } else {
            &scheduled_resources
        },
        resource_sets: if matches!(dispatch, DispatchCase::FlowSharedCleanup) {
            &SharedProfile
        } else {
            &Profile
        },
        operations: &operations,
        provider: &UnusedProvider,
        mutation: &UnusedJournal,
    };
    if matches!(dispatch, DispatchCase::PublicLoop) {
        exercise_public_loop(
            &listener,
            &service,
            &authority,
            operations.clone(),
            clock.clone(),
            scheduler.clone(),
            (subject, authenticator),
            path,
        );
        std::fs::remove_file(socket_path).unwrap();
        return;
    }
    let client = ClientEnd {
        worker: worker.unwrap(),
    };
    // A positive scenario reads the result by a real PAM client; a pause after Accepted
    // allows you to experience socket overflow separately.
    let mut positive_client = None;
    let mut positive_resume = None;
    let mut positive_accepted = None;
    // Adapter test clients use a common public socket; authority in this composition remains
    // test.
    let separate_client =
        tunnel_mode && std::env::var_os("CAROSIGN_FLOW_SEPARATE_CLIENT").is_some();
    let adapter_mode = tunnel_mode
        && matches!(dispatch, DispatchCase::Normal)
        && (separate_client
            || std::env::var_os("CAROSIGN_TEST_FPRINTD_ADAPTER").is_some()
            || std::env::var_os("CAROSIGN_TEST_PAM_OBSERVER").is_some());
    let mut adapter_client = if adapter_mode {
        let fd = client.worker.try_clone().unwrap();
        Some(std::thread::spawn(move || {
            if separate_client {
                let (mut child, _sync) = spawn_flow_client_process(fd, false);
                assert!(child.wait().unwrap().success());
            } else {
                run_public_client(fd, authenticator)
            }
        }))
    } else {
        None
    };
    if matches!(
        dispatch,
        DispatchCase::FlowMatched | DispatchCase::FlowScoped
    ) {
        let fd = client.worker.try_clone().unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        positive_accepted = Some(ready_rx);
        positive_resume = Some(resume_tx);
        positive_client = Some(std::thread::spawn(move || {
            if separate_client {
                use std::io::{Read, Write};
                let (mut child, mut sync) = spawn_flow_client_process(fd, true);
                let mut operation = [0u8; 16];
                sync.read_exact(&mut operation).unwrap();
                ready_tx.send(operation.to_vec()).unwrap();
                resume_rx.recv().unwrap();
                sync.write_all(&[1]).unwrap();
                assert!(child.wait().unwrap().success());
                return Ok(pam_carosign::client::Observation::FactorAccepted);
            }
            let transport =
                carosign_linux_ipc::client::transport::Transport::from_connected(fd).unwrap();
            let request = client::BeginVerification {
                report_matched_enrollment: false,
                enrollment: None,
                idempotency_key: vec![12; 16],
                authenticator_id: authenticator.as_bytes().to_vec(),
                subject: Some(carosign_protocol::carosign::common::v1::SubjectSelector {
                    selector: Some(carosign_protocol::carosign::common::v1::subject_selector::Selector::CurrentSubject(true)),
                }),
                purpose_hint: carosign_protocol::carosign::common::v1::VerificationPurpose::PurposeSessionLogin as i32,
                authority_request_ref: vec![9, 8],
            };
            pam_carosign::client::observe_verification_with_acceptance(&transport, request, |id| {
                ready_tx.send(id.to_vec()).unwrap();
                resume_rx.recv().unwrap();
            })
        }));
    }
    std::fs::remove_file(socket_path).unwrap();
    drop(listener);
    let counted = counted.as_mut().unwrap();
    let connection_handle = counted.handle;
    let mut connection = &mut counted.connection;
    let send_frame = |frame: client::ClientFrame| {
        let bytes = frame.encode_to_vec();
        // SAFETY: The socket and buffer remain valid throughout the call.
        let size = unsafe {
            libc::send(
                client.worker.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                0,
            )
        };
        assert_eq!(size, bytes.len() as isize);
    };
    if positive_client.is_none() && !adapter_mode {
        send_frame(client::ClientFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_MAX,
            request_id: vec![1; 16],
            operation_id: Vec::new(),
            event_sequence: 0,
            payload: Some(client::client_frame::Payload::Hello(client::Hello {
                capture_feedback_v1: false,
                major: 1,
                min_minor: carosign_protocol::limits::MINOR_BASELINE,
                max_minor: carosign_protocol::limits::MINOR_MAX,
            })),
        });
    }
    let mut buffer = vec![0; carosign_protocol::limits::FRAME_MAX];
    let len = crate::server::read_frame(&connection, &mut buffer)
        .unwrap()
        .unwrap();
    let hello = crate::server::greet(&mut connection, &buffer[..len]).unwrap();
    crate::server::write_frame(&connection, &hello).unwrap();
    // SAFETY: The socket and buffer remain valid throughout the call.
    if positive_client.is_none() && !adapter_mode {
        assert!(
            unsafe {
                libc::recv(
                    client.worker.as_raw_fd(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    0,
                )
            } > 0
        );
    }
    if positive_client.is_none() && !adapter_mode {
        send_frame(client::ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: vec![2; 16],
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(client::client_frame::Payload::BeginVerification(client::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: vec![12; 16],
            authenticator_id: authenticator.as_bytes().to_vec(),
            subject: Some(carosign_protocol::carosign::common::v1::SubjectSelector {
                selector: Some(carosign_protocol::carosign::common::v1::subject_selector::Selector::CurrentSubject(true)),
            }),
            purpose_hint: carosign_protocol::carosign::common::v1::VerificationPurpose::PurposeSessionLogin as i32,
            authority_request_ref: vec![9, 8],
        })),
    });
    }
    let len = crate::server::read_frame(&connection, &mut buffer)
        .unwrap()
        .unwrap();
    let begin_request_id = client::ClientFrame::decode(&buffer[..len])
        .unwrap()
        .request_id;
    let decoded = connection.session.accept(&buffer[..len]).unwrap();
    let domain = carosign_linux_ipc::mapping::client::to_domain(&decoded)
        .unwrap()
        .unwrap();
    let prepared = futures_executor::block_on(
        service.prepare_client_verification(&caller, &domain, &authority, epoch),
    )
    .unwrap();
    let GrantScope::Verify { purpose, .. } = *prepared.grant.scope() else {
        panic!("не область проверки")
    };
    assert_eq!(
        purpose,
        carosign_core::operation::context::VerificationPurpose::SessionUnlock
    );
    let binding = crate::operations::TrustedCallerBinding::fixture([42; 32]);
    if matches!(
        dispatch,
        DispatchCase::Normal
            | DispatchCase::FlowTimeout
            | DispatchCase::FlowBlocked
            | DispatchCase::FlowRevoked
            | DispatchCase::FlowDisconnected
            | DispatchCase::FlowSharedCleanup
            | DispatchCase::FlowMatched
            | DispatchCase::FlowScoped
    ) && !uncertain
    {
        use crate::verification_owner::{
            FlowError, FlowProgress, VerificationFlow, VerificationFlowTarget,
        };
        // The test owner knows in advance the ID issued by the fixture; a random challenge
        // creates a real limited registry of the platform.
        // This context and source of origin are only testable.
        let scoped_challenges = crate::scoped_challenge::ScopedChallenges::new(4);
        let scoped_context = matches!(dispatch, DispatchCase::FlowScoped).then(|| {
            scoped_challenges
                .issue(
                    Ids.new_operation_id().unwrap(),
                    carosign_runtime::ports::ScopedAssertionContext {
                        subject,
                        authenticator: AuthenticatorRef {
                            id: authenticator,
                            device_epoch: epoch,
                        },
                        purpose,
                        recipient: carosign_evidence::recipient::RecipientRef::new([23; 16])
                            .unwrap(),
                        challenge:
                            carosign_evidence::recipient::ChallengeRef::accepted_by_recipient(
                                [22; 16],
                            )
                            .unwrap(),
                        deadline: carosign_core::operation::context::BootDeadline::new(
                            boot,
                            30_000_000_000,
                        ),
                    },
                    clock.now(),
                )
                .unwrap()
        });
        let target = VerificationFlowTarget {
            worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
            command: CommandId::from_bytes([2; 16]).unwrap(),
            authenticator: AuthenticatorRef {
                id: authenticator,
                device_epoch: epoch,
            },
            provider_subject: subject,
            uid: 1000,
            purpose,
            challenge: scoped_context.as_ref().map_or_else(
                || {
                    carosign_evidence::recipient::ChallengeRef::accepted_by_recipient([22; 16])
                        .unwrap()
                },
                |context| context.challenge,
            ),
            recipient: carosign_evidence::recipient::RecipientRef::new([23; 16]).unwrap(),
        };
        let foreign = CallerContext::from_transport(
            [98; 32],
            uid,
            TransactionId::from_bytes([98; 16]).unwrap(),
        );
        let control = std::sync::Arc::new(crate::control_watch::ControlOwner::default());
        let _policy_lock =
            matches!(dispatch, DispatchCase::FlowRevoked).then(crate::policy_test_lock);
        let policy_dir = if matches!(dispatch, DispatchCase::FlowRevoked) {
            let path = std::path::Path::new("/dev/shm")
                .join(format!("carosign-flow-policy-{}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("policy"), b"first").unwrap();
            Some(path)
        } else {
            None
        };
        let policy_watch = policy_dir.as_ref().map(|path| {
            carosign_linux::policy::PolicyChangeWatch::new(&path.join("policy")).unwrap()
        });
        let mut flow = VerificationFlow::admit_observing(
            &service,
            &prepared,
            Some(&binding),
            connection_handle,
            8,
            operations.clone(),
            clock.clone(),
            control.clone(),
            scheduler.clone(),
            target,
            Duration::from_secs(5),
            policy_watch,
        )
        .unwrap();
        let operation = flow.operation();
        if scoped_context.is_some() {
            assert_eq!(operation, Ids.new_operation_id().unwrap());
        }
        // The transaction number and duration are given to the client before the result of
        // the provider.
        let accepted = crate::answer::encode(&flow.accepted_reply().unwrap(), &begin_request_id);
        crate::server::write_frame(&connection, &accepted).unwrap();
        if let Some(ready) = positive_accepted.take() {
            assert_eq!(
                ready.recv_timeout(Duration::from_secs(5)).unwrap(),
                operation.as_bytes()
            );
        } else if !adapter_mode {
            let mut accepted_bytes = vec![0; 65536];
            // SAFETY: The socket and buffer act while reading.
            let size = unsafe {
                libc::recv(
                    client.worker.as_raw_fd(),
                    accepted_bytes.as_mut_ptr().cast(),
                    accepted_bytes.len(),
                    0,
                )
            };
            assert!(size > 0);
            let accepted_frame =
                client::ClientFrame::decode(&accepted_bytes[..size as usize]).unwrap();
            assert_eq!(accepted_frame.request_id, vec![2; 16]);
            assert!(matches!(accepted_frame.payload,
            Some(client::client_frame::Payload::OperationAccepted(ref accepted))
                if accepted.operation_id == operation.as_bytes()
                    && accepted.deadline.as_ref().is_some_and(|deadline|
                        deadline.boot_id == boot.as_bytes() && deadline.boottime_ns == 60_000_000_000)));
        }
        assert!(matches!(VerificationFlow::admit(
            &service, &prepared, Some(&binding), connection_handle, 8,
            operations.clone(), clock.clone(), control.clone(), scheduler.clone(), target, Duration::from_secs(5),
        ), Err(FlowError::Admission(crate::operations::IdempotentAdmission::Existing(id))) if id == operation));
        let context = VerificationFrameContext {
            worker: target.worker,
            command: target.command,
            operation,
            authenticator: target.authenticator,
            provider_subject: target.provider_subject,
            uid: target.uid,
            purpose: target.purpose,
            challenge: target.challenge,
            recipient: target.recipient,
        };
        assert!(matches!(
            VerificationFlow::new(
                operations.clone(),
                clock.clone(),
                control.clone(),
                scheduler.clone(),
                context.clone(),
                foreign,
                Duration::from_secs(5),
            ),
            Err(FlowError::Operation(
                crate::operations::ObservationFailure::ContextMismatch
            ))
        ));
        assert!(matches!(
            VerificationFlow::new(
                operations.clone(),
                clock.clone(),
                control,
                scheduler.clone(),
                context,
                caller.clone(),
                Duration::from_secs(5),
            ),
            Err(FlowError::Watch(
                crate::control_watch::StartError::Duplicate
            ))
        ));
        let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
        if matches!(dispatch, DispatchCase::FlowDisconnected) {
            // Closing common copies of the descriptor in parallel fork is not instantaneous;
            // shutdown breaks the connection.
            // SAFETY: The test owns the descriptor and keeps it valid until closing it.
            assert_eq!(
                unsafe { libc::shutdown(sockets.worker.as_raw_fd(), libc::SHUT_RDWR) },
                0
            );
            drop(sockets.worker);
            let transport = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
            assert!(matches!(
                futures_executor::block_on(flow.run(
                    &service,
                    &prepared,
                    Some(transport),
                    &connection,
                    &Cleanup,
                    Duration::from_secs(2),
                    std::future::pending,
                )),
                Ok(FlowProgress::DispatchFailure(
                    crate::effect_owner::DriveOutcome::ExternalEffect
                ))
            ));
            assert_eq!(
                operations.phase(operation),
                Some(OperationPhase::Cancelling)
            );
            assert!(operations.has_retained_lease(operation));
            return;
        }
        if matches!(dispatch, DispatchCase::FlowRevoked) {
            let policy_dir = policy_dir.unwrap();
            std::fs::write(policy_dir.join("policy"), b"changed").unwrap();
            let end = std::time::Instant::now() + Duration::from_secs(2);
            while !flow.control_observed() {
                assert!(std::time::Instant::now() < end, "отзыв не обработан");
                std::thread::yield_now();
            }
            assert!(flow.accepted_reply().is_err());
            let transport = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
            assert!(
                futures_executor::block_on(flow.run(
                    &service,
                    &prepared,
                    Some(transport),
                    &connection,
                    &Cleanup,
                    Duration::from_secs(2),
                    std::future::pending,
                ))
                .is_err()
            );
            assert_no_packet(&sockets.worker);
            std::fs::remove_dir_all(policy_dir).unwrap();
            return;
        }
        if matches!(dispatch, DispatchCase::FlowBlocked) {
            let fill = [0u8; 1024];
            let mut full = false;
            for _ in 0..16_384 {
                // SAFETY: The socket and buffer remain valid for the nonblocking send.
                let n = unsafe {
                    libc::send(
                        connection.fd.as_raw_fd(),
                        fill.as_ptr().cast(),
                        fill.len(),
                        libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                    )
                };
                if n == -1 {
                    assert_eq!(
                        std::io::Error::last_os_error().raw_os_error(),
                        Some(libc::EAGAIN)
                    );
                    full = true;
                    break;
                }
                assert_eq!(n, fill.len() as isize);
            }
            assert!(full);
        }
        let mut relay_process = None;
        let (transport, worker, mut process) = if tunnel_mode {
            let binary = std::env::var_os("CAROSIGN_MOCK_PROVIDER_TEST_BINARY")
                .expect("для tunnel нужен отдельный mock-провайдер");
            let (transport, child, broker) = prepare_tunnel_mock_process(
                std::path::Path::new(&binary),
                sockets,
                if matches!(dispatch, DispatchCase::FlowScoped) {
                    "seeded-matched-scope"
                } else if matches!(dispatch, DispatchCase::FlowMatched) {
                    "seeded-matched-token"
                } else {
                    "seeded-no-sample"
                },
            );
            relay_process = Some(broker);
            (transport, None, Some(child))
        } else if matches!(
            dispatch,
            DispatchCase::FlowMatched | DispatchCase::FlowScoped
        ) {
            let binary = std::env::var_os("CAROSIGN_MOCK_PROVIDER_TEST_BINARY")
                .expect("для положительной проверки нужен отдельный mock-провайдер");
            let (transport, child) = prepare_mock_process_scenario(
                std::path::Path::new(&binary),
                sockets,
                if matches!(dispatch, DispatchCase::FlowScoped) {
                    "matched-scope"
                } else if seeded {
                    "seeded-matched-token"
                } else {
                    "matched-token"
                },
            );
            (
                crate::provider_verification::VerificationTransport::Direct(transport),
                None,
                Some(child),
            )
        } else {
            let (transport, worker, process) =
                prepare_flow_provider(sockets, matches!(dispatch, DispatchCase::FlowTimeout));
            (
                crate::provider_verification::VerificationTransport::Direct(transport),
                worker,
                process,
            )
        };
        let shared_cleanup = SharedCleanup {
            scheduler: &scheduler,
            boot,
        };
        let cleanup: &dyn crate::resources::TrustedCleanupBroker =
            if matches!(dispatch, DispatchCase::FlowSharedCleanup) {
                &shared_cleanup
            } else {
                &Cleanup
            };
        if matches!(dispatch, DispatchCase::Normal | DispatchCase::FlowTimeout) {
            let scope =
                futures_executor::block_on(flow.prepare_channel(&service, &prepared)).unwrap();
            assert_eq!(scope.owner().operation, operation);
            assert_eq!(scope.owner().authenticator.id, authenticator);
        }
        let result = futures_executor::block_on(flow.run_channel(
            &service,
            &prepared,
            Some(transport),
            &connection,
            cleanup,
            Duration::from_secs(2),
            std::future::pending,
        ));
        if matches!(dispatch, DispatchCase::FlowTimeout) {
            assert!(
                matches!(result, Ok(FlowProgress::Sent)),
                "тайм-аут: {result:?}"
            );
            assert!(!operations.has_retained_lease(operation));
            if let Some(worker) = worker {
                worker.join().unwrap();
            }
            return;
        }
        if matches!(dispatch, DispatchCase::FlowScoped) {
            assert!(
                matches!(result, Ok(FlowProgress::Other(_))),
                "итог: {result:?}"
            );
            use scoped_verifier::ScopedVerifier;
            let mut requirements = carosign_evidence::requirements::EvidenceRequirements {
                purpose,
                hardware_required: true,
                fresh_challenge: true,
            };
            let verifier = ScopedVerifier {
                store: operations.assertion_store(),
                clock: clock.as_ref(),
                challenges: &scoped_challenges,
                expected_operation: operation,
                expected_worker: target.worker,
                expected_command: target.command,
            };
            let mut wrong_context = scoped_context.clone().unwrap();
            wrong_context.recipient =
                carosign_evidence::recipient::RecipientRef::new([24; 16]).unwrap();
            assert_eq!(
                scoped_challenges.consume(operation, &wrong_context, clock.now()),
                Err(crate::scoped_challenge::ChallengeError::Mismatch)
            );
            assert!(matches!(
                futures_executor::block_on(
                    flow.evaluate_scoped_assertion(&verifier, &requirements)
                ),
                Err(FlowError::ContextMismatch)
            ));
            requirements.hardware_required = false;
            let assessment = futures_executor::block_on(
                flow.evaluate_scoped_assertion(&verifier, &requirements),
            )
            .unwrap();
            assert_eq!(
                scoped_challenges.consume(operation, scoped_context.as_ref().unwrap(), clock.now()),
                Err(crate::scoped_challenge::ChallengeError::Consumed)
            );
            assert_eq!(assessment.assertion().operation, operation);
            assert!(assessment.assertion().matched.is_none());
            assert_eq!(assessment.evidence().as_bytes(), &[11; 16]);
            assert_eq!(assessment.context().purpose, purpose);
            assert_eq!(
                assessment.class(),
                carosign_core::assurance::AssuranceClass::SoftwareAsserted
            );
            assert!(matches!(
                futures_executor::block_on(
                    flow.evaluate_scoped_assertion(&verifier, &requirements)
                ),
                Err(FlowError::InvalidStage)
            ));
            assert_eq!(operations.phase(operation), Some(OperationPhase::Running));
            let assertion_evidence_id = assessment.evidence();
            assert_no_packet(&client.worker);
            futures_executor::block_on(flow.prepare_scoped_positive(
                assessment,
                &authority,
                cleanup,
                std::future::pending,
            ))
            .unwrap();
            let mut completion = flow.start_positive_delivery().unwrap();
            positive_resume.take().unwrap().send(()).unwrap();
            assert_eq!(
                futures_executor::block_on(completion.drive(
                    &connection,
                    &authority,
                    clock.as_ref(),
                ))
                .unwrap(),
                crate::operations::OwnerSendProgress::Sent
            );
            assert_eq!(
                positive_client.take().unwrap().join().unwrap().unwrap(),
                pam_carosign::client::Observation::FactorAccepted
            );
            assert!(
                operations
                    .assertion_store()
                    .reference_for(operation, assertion_evidence_id)
                    .is_err()
            );
            assert_no_packet(&client.worker);
            drop(completion);
            drop(flow);
            return;
        }
        if matches!(dispatch, DispatchCase::FlowMatched) {
            assert!(
                matches!(result, Ok(FlowProgress::Other(_))),
                "итог: {result:?}"
            );
            use test_verifier::TestVerifier;
            let verifier = TestVerifier {
                registry: &operations,
                clock: &clock,
            };
            futures_executor::block_on(flow.evaluate_matched(
                &Ids,
                &verifier,
                &carosign_evidence::requirements::EvidenceRequirements {
                    purpose,
                    hardware_required: false,
                    fresh_challenge: true,
                },
                cleanup,
                std::future::pending,
            ))
            .unwrap();
            futures_executor::block_on(flow.prepare_positive(
                &authority,
                cleanup,
                std::future::pending,
            ))
            .unwrap();
            let mut completion = flow.start_positive_delivery().unwrap();
            let denied = ClientAuthority {
                caller: caller.clone(),
                subject,
                boot,
                refusal: Some(AuthorityRefusal::Revoked),
            };
            assert!(
                futures_executor::block_on(completion.drive(&connection, &denied, clock.as_ref(),))
                    .is_err()
            );
            assert_no_packet(&client.worker);
            let filler = [0xa5u8; 1024];
            loop {
                // SAFETY: The test owns the socket and buffer.
                let size = unsafe {
                    libc::send(
                        connection.fd.as_raw_fd(),
                        filler.as_ptr().cast(),
                        filler.len(),
                        libc::MSG_DONTWAIT,
                    )
                };
                if size < 0 {
                    assert_eq!(
                        std::io::Error::last_os_error().raw_os_error(),
                        Some(libc::EAGAIN)
                    );
                    break;
                }
                assert_eq!(size as usize, filler.len());
            }
            assert_eq!(
                futures_executor::block_on(completion.drive(
                    &connection,
                    &authority,
                    clock.as_ref()
                ))
                .unwrap(),
                crate::operations::OwnerSendProgress::WouldBlock,
            );
            loop {
                let mut bytes = [0u8; 1024];
                // SAFETY: The client and the buffer belong to the test.
                let size = unsafe {
                    libc::recv(
                        client.worker.as_raw_fd(),
                        bytes.as_mut_ptr().cast(),
                        bytes.len(),
                        libc::MSG_DONTWAIT,
                    )
                };
                if size < 0 {
                    assert_eq!(
                        std::io::Error::last_os_error().raw_os_error(),
                        Some(libc::EAGAIN)
                    );
                    break;
                }
                assert_eq!(size as usize, filler.len());
                assert_eq!(bytes, filler);
            }
            positive_resume.take().unwrap().send(()).unwrap();
            assert_eq!(
                futures_executor::block_on(completion.drive(
                    &connection,
                    &authority,
                    clock.as_ref()
                ))
                .unwrap(),
                crate::operations::OwnerSendProgress::Sent
            );
            assert_eq!(
                positive_client.take().unwrap().join().unwrap().unwrap(),
                pam_carosign::client::Observation::FactorAccepted
            );
            assert_no_packet(&client.worker);
            drop(completion);
            drop(flow);
            if let Some(child) = process.as_mut() {
                assert!(
                    child
                        .wait_bounded(Duration::from_secs(5))
                        .unwrap()
                        .success()
                );
            }
            if let Some(child) = relay_process.as_mut() {
                assert!(
                    child
                        .wait_bounded(Duration::from_secs(5))
                        .unwrap()
                        .success()
                );
            }
            return;
        }
        if matches!(dispatch, DispatchCase::FlowSharedCleanup) {
            assert_eq!(
                scheduler.lock().unwrap().take_cleanup_grants(),
                vec![(
                    LeaseId::from_bytes([96; 16]).unwrap(),
                    OperationId::from_bytes([95; 16]).unwrap()
                )]
            );
        }
        if matches!(dispatch, DispatchCase::FlowBlocked) {
            assert!(
                matches!(result, Ok(FlowProgress::WouldBlock)),
                "результат: {result:?}"
            );
            let mut trash = [0u8; 2048];
            loop {
                // SAFETY: The socket and buffer remain valid for the nonblocking read.
                let n = unsafe {
                    libc::recv(
                        client.worker.as_raw_fd(),
                        trash.as_mut_ptr().cast(),
                        trash.len(),
                        libc::MSG_DONTWAIT,
                    )
                };
                if n < 0 {
                    assert_eq!(
                        std::io::Error::last_os_error().raw_os_error(),
                        Some(libc::EAGAIN)
                    );
                    break;
                }
                assert!(n > 0);
            }
            assert!(matches!(
                futures_executor::block_on(flow.run(
                    &service,
                    &prepared,
                    None,
                    &connection,
                    &Cleanup,
                    Duration::from_secs(2),
                    std::future::pending,
                )),
                Ok(FlowProgress::Sent)
            ));
        } else {
            assert!(
                matches!(result, Ok(FlowProgress::Sent)),
                "результат: {result:?}"
            );
        }
        assert!(matches!(
            futures_executor::block_on(flow.run(
                &service,
                &prepared,
                None,
                &connection,
                &Cleanup,
                Duration::ZERO,
                std::future::pending,
            )),
            Err(FlowError::InvalidStage)
        ));
        assert!(matches!(
            flow.accepted_reply(),
            Err(FlowError::InvalidStage)
        ));
        if let Some(worker) = worker {
            worker.join().unwrap();
        }
        if let Some(child) = process.as_mut() {
            assert!(
                child
                    .wait_bounded(Duration::from_secs(5))
                    .unwrap()
                    .success()
            );
        }
        if let Some(child) = relay_process.as_mut() {
            assert!(
                child
                    .wait_bounded(Duration::from_secs(5))
                    .unwrap()
                    .success()
            );
        }
        if let Some(client) = adapter_client.take() {
            client.join().unwrap();
        } else {
            read_no_sample_until_terminal(client.worker.as_raw_fd());
        }
        return;
    }
    let crate::operations::IdempotentAdmission::Created(operation) = service
        .admit_client_verification(
            &prepared,
            DeviceEpoch::FIRST,
            Some(&binding),
            connection_handle,
            8,
        )
    else {
        panic!("операция не опубликована")
    };
    // Supervision begins before the request for issuance and is valid until delivery.
    let control = std::sync::Arc::new(crate::control_watch::ControlOwner::default());
    let mut watched = if matches!(
        dispatch,
        DispatchCase::Normal
            | DispatchCase::BusyStartOwner
            | DispatchCase::StartDisconnected
            | DispatchCase::StartRevoked
            | DispatchCase::StartControlRevoked
    ) {
        Some(
            control
                .start(
                    std::sync::Arc::clone(&operations),
                    operation,
                    clock.clone(),
                    Duration::from_secs(5),
                )
                .unwrap(),
        )
    } else {
        None
    };
    let foreign = crate::service::PreparedClientVerification {
        caller: CallerContext::from_transport(
            [98; 32],
            uid,
            TransactionId::from_bytes([98; 16]).unwrap(),
        ),
        idempotency: prepared.idempotency,
        intent: prepared.intent.clone(),
        cycle: prepared.cycle.clone(),
        grant: prepared.grant.clone(),
    };
    assert!(matches!(
        futures_executor::block_on(service.prepare_admitted_dispatch(
            &foreign,
            operation,
            WorkerInstanceId::from_bytes([1; 16]).unwrap()
        )),
        Err(crate::operations::ObservationFailure::ContextMismatch)
    ));
    let outcome = futures_executor::block_on(service.prepare_admitted_dispatch(
        &prepared,
        operation,
        WorkerInstanceId::from_bytes([1; 16]).unwrap(),
    ));
    if matches!(
        dispatch,
        DispatchCase::ForeignLease | DispatchCase::ForeignWorker
    ) {
        assert!(matches!(
            outcome,
            Err(crate::operations::ObservationFailure::ContextMismatch)
        ));
        assert!(operations.has_retained_lease(operation));
        return;
    }
    if matches!(dispatch, DispatchCase::RevokeDuringAcquire) {
        assert!(matches!(
            outcome,
            Err(crate::operations::ObservationFailure::AwaitingActor)
        ));
        assert!(operations.has_retained_lease(operation));
        return;
    }
    if matches!(dispatch, DispatchCase::ExpireDuringAcquire) {
        assert!(matches!(
            outcome,
            Err(crate::operations::ObservationFailure::DeadlineExpired)
        ));
        assert!(operations.has_retained_lease(operation));
        return;
    }
    let outcome = outcome.unwrap();
    assert!(matches!(
        outcome,
        crate::run::RunOutcome::Waiting {
            phase: OperationPhase::Running
        }
    ));
    assert!(operations.has_retained_lease(operation));
    assert!(
        futures_executor::block_on(service.prepare_admitted_dispatch(
            &prepared,
            operation,
            WorkerInstanceId::from_bytes([1; 16]).unwrap()
        ),)
        .is_err()
    );
    let command = CommandId::from_bytes([2; 16]).unwrap();
    let context = VerificationFrameContext {
        worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
        command,
        operation,
        authenticator: AuthenticatorRef {
            id: authenticator,
            device_epoch: DeviceEpoch::FIRST,
        },
        provider_subject: subject,
        uid: 1000,
        purpose,
        challenge: carosign_evidence::recipient::ChallengeRef::accepted_by_recipient([22; 16])
            .unwrap(),
        recipient: carosign_evidence::recipient::RecipientRef::new([23; 16]).unwrap(),
    };
    let dispatch_authority = ClientAuthority {
        caller: caller.clone(),
        subject,
        boot,
        refusal: matches!(dispatch, DispatchCase::Revoked | DispatchCase::StartRevoked)
            .then_some(AuthorityRefusal::Revoked),
    };
    if matches!(dispatch, DispatchCase::Expired) {
        clock.advance(61_000_000_000);
    }
    let mut starter = if matches!(
        dispatch,
        DispatchCase::Normal
            | DispatchCase::BusyStartOwner
            | DispatchCase::StartDisconnected
            | DispatchCase::StartRevoked
            | DispatchCase::StartControlRevoked
    ) {
        Some(VerificationStart::bind(&operations, context.clone()).unwrap())
    } else {
        None
    };
    let effects = EffectOwner::default();
    let start_ports = crate::verification_owner::VerificationStartPorts {
        registry: &operations,
        effects: &effects,
        authority: &dispatch_authority,
        clock: clock.as_ref(),
        broker: &Cleanup,
        scheduler: &scheduler,
    };
    if starter.is_none() {
        let (binding_step, prepared_frame) = futures_executor::block_on(
            operations.bind_and_prepare_verification(&context, clock.as_ref(), &dispatch_authority),
        )
        .unwrap();
        assert!(matches!(binding_step, Step::Advanced { .. }));
        effects.accept_step(operation, &binding_step).unwrap();
        assert!(matches!(
            futures_executor::block_on(effects.drive_negative(
                &operations,
                operation,
                &Cleanup,
                std::future::pending,
            )),
            Ok(crate::effect_owner::DriveOutcome::ExternalEffect)
        ));
        match dispatch {
            DispatchCase::Revoked => {
                assert!(matches!(
                    prepared_frame,
                    Err(
                        crate::operations::VerificationPreparationFailure::Authority(
                            AuthorityRefusal::Revoked
                        )
                    )
                ));
                return;
            }
            DispatchCase::Expired => {
                assert!(matches!(
                    prepared_frame,
                    Err(
                        crate::operations::VerificationPreparationFailure::Operation(
                            crate::operations::ObservationFailure::DeadlineExpired
                        )
                    )
                ));
                return;
            }
            DispatchCase::Normal
            | DispatchCase::PublicLoop
            | DispatchCase::Disconnected
            | DispatchCase::Dropped
            | DispatchCase::InvalidFrame
            | DispatchCase::CancelDuringSend
            | DispatchCase::CancelAfterUnknown
            | DispatchCase::ForeignLease
            | DispatchCase::ForeignWorker
            | DispatchCase::RevokeDuringAcquire
            | DispatchCase::ExpireDuringAcquire
            | DispatchCase::RevokeBeforeDelivery
            | DispatchCase::ExpireBeforeDelivery
            | DispatchCase::FullDeliverySocket
            | DispatchCase::RevokedWhileDeliveryBlocked
            | DispatchCase::PolicyRevokedWhileDeliveryBlocked
            | DispatchCase::RevokeDuringDeliveryPolicy
            | DispatchCase::LostDelivery
            | DispatchCase::LostPartialDelivery
            | DispatchCase::LostBlockedDelivery
            | DispatchCase::RevokedAfterDeliveryLoss => {}
            DispatchCase::BusyStartOwner
            | DispatchCase::StartDisconnected
            | DispatchCase::StartRevoked
            | DispatchCase::StartControlRevoked => {}
            DispatchCase::FlowTimeout
            | DispatchCase::FlowBlocked
            | DispatchCase::FlowRevoked
            | DispatchCase::FlowDisconnected
            | DispatchCase::FlowSharedCleanup
            | DispatchCase::FlowMatched
            | DispatchCase::FlowScoped => unreachable!(),
        }
        let (_frame, _permitted) = prepared_frame.unwrap();
    }
    if matches!(
        dispatch,
        DispatchCase::CancelDuringSend | DispatchCase::CancelAfterUnknown
    ) {
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec};
        use carosign_runtime::OperationMsg;
        let mut forged = context.clone();
        forged.uid = 0;
        assert!(matches!(
            futures_executor::block_on(operations.begin_revalidated_verification_send(
                &forged,
                clock.as_ref(),
                &dispatch_authority,
            )),
            Err(
                crate::operations::VerificationPreparationFailure::Operation(
                    crate::operations::ObservationFailure::ContextMismatch
                )
            )
        ));
        let attempt = futures_executor::block_on(operations.begin_revalidated_verification_send(
            &context,
            clock.as_ref(),
            &dispatch_authority,
        ))
        .unwrap();
        let mut attempt = Some(attempt);
        if matches!(dispatch, DispatchCase::CancelAfterUnknown) {
            drop(attempt.take());
        }
        let step = operations.revoke_verification(operation).unwrap();
        assert!(
            matches!(&step, Step::Advanced { state, .. } if state.common.phase == OperationPhase::Cancelling)
        );
        let delivery = VerificationContext {
            operation,
            worker: context.worker,
            command: context.command,
            authenticator: context.authenticator.id,
            epoch: context.authenticator.device_epoch,
        };
        if matches!(dispatch, DispatchCase::CancelAfterUnknown) {
            assert!(matches!(
                operations.deliver_verification_send_failure(&delivery),
                Err(crate::operations::ObservationFailure::AwaitingActor)
            ));
        }
        let mut guard = operations.checkout(operation).unwrap();
        let actor = &mut guard.operation_mut().actor;
        let cancel = *actor
            .pending()
            .iter()
            .find(|effect| effect.spec == EffectSpec::RequestCancel)
            .unwrap();
        actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect: cancel,
                receipt: EffectReceipt::CancelDispatched,
            }))
            .unwrap();
        let step = actor.run_once();
        let Step::Advanced {
            effects: ref next, ..
        } = step
        else {
            panic!("ожидалась отмена")
        };
        let cleanup = *next
            .iter()
            .find(|effect| effect.spec == EffectSpec::ReleaseResources)
            .unwrap();
        drop(guard);
        if matches!(dispatch, DispatchCase::CancelAfterUnknown) {
            let foreign = VerificationContext {
                worker: WorkerInstanceId::from_bytes([99; 16]).unwrap(),
                ..delivery
            };
            assert!(matches!(
                operations.deliver_verification_send_failure(&foreign),
                Err(crate::operations::ObservationFailure::ContextMismatch)
            ));
            let step = operations
                .deliver_verification_send_failure(&delivery)
                .unwrap();
            assert!(matches!(&step, Step::Advanced { state, effects }
                if state.common.phase == OperationPhase::Finalizing
                && effects.iter().any(|effect| effect.spec == EffectSpec::QuarantineScope)
                && !state.common.pending_effects.iter().any(|effect| effect.spec == EffectSpec::ReleaseResources)));
            assert!(operations.has_retained_lease(operation));
            assert!(
                operations
                    .deliver_verification_send_failure(&delivery)
                    .is_err()
            );
            let blocked = EffectOwner::default();
            blocked.accept_step(operation, &step).unwrap();
            assert!(matches!(
                operations.handoff_verification_send_failure(operation, &blocked),
                Err(crate::effect_owner::EffectOwnerError::Duplicate)
            ));
            operations
                .handoff_verification_send_failure(operation, &effects)
                .unwrap();
            assert!(matches!(
                effects.step(operation),
                Some(Step::Advanced { .. })
            ));
            return;
        }
        let result = futures_executor::block_on(
            operations.cleanup_verification(operation, cleanup, &Cleanup),
        )
        .unwrap();
        assert!(
            matches!(result, crate::operations::CleanupProgress::Uncertain(Step::Advanced { ref effects, .. })
            if effects.iter().any(|effect| effect.spec == EffectSpec::QuarantineScope))
        );
        assert!(operations.has_retained_lease(operation));
        if let Some(attempt) = attempt {
            let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
            let transport = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
            assert!(matches!(
                attempt.send(transport, Duration::from_secs(1)),
                Err(crate::provider_verification::VerificationError::Protocol)
            ));
            let mut byte = [0u8; 1];
            // SAFETY: The descriptor and buffer remain valid.
            assert_eq!(
                unsafe {
                    libc::recv(
                        sockets.worker.as_raw_fd(),
                        byte.as_mut_ptr().cast(),
                        1,
                        libc::MSG_DONTWAIT,
                    )
                },
                0
            );
        }
        return;
    }
    if matches!(
        dispatch,
        DispatchCase::StartDisconnected
            | DispatchCase::StartRevoked
            | DispatchCase::StartControlRevoked
    ) {
        let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let transport = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
        if matches!(dispatch, DispatchCase::StartDisconnected) {
            // SAFETY: The test owns the descriptor and keeps it valid until closing it.
            assert_eq!(
                unsafe { libc::shutdown(sockets.worker.as_raw_fd(), libc::SHUT_RDWR) },
                0
            );
            drop(sockets.worker);
            assert!(matches!(
                futures_executor::block_on(starter.as_mut().unwrap().start(
                    &start_ports,
                    transport,
                    Duration::from_secs(1),
                    std::future::pending,
                )),
                Err(crate::verification_owner::StartError::Send(
                    crate::provider_verification::VerificationError::Transport(_)
                ))
            ));
            let delivery = VerificationContext {
                operation,
                worker: context.worker,
                command: context.command,
                authenticator: context.authenticator.id,
                epoch: context.authenticator.device_epoch,
            };
            let step = operations
                .deliver_verification_send_failure(&delivery)
                .unwrap();
            assert!(matches!(step, Step::Advanced { ref state, ref effects }
                if state.common.phase == OperationPhase::Cancelling
                && effects.iter().any(|effect| effect.spec == carosign_engine::EffectSpec::QuarantineScope)));
            assert!(operations.has_retained_lease(operation));
        } else if matches!(dispatch, DispatchCase::StartControlRevoked) {
            let _ = operations.revoke_verification(operation);
            assert!(matches!(
                futures_executor::block_on(starter.as_mut().unwrap().start(
                    &start_ports,
                    transport,
                    Duration::from_secs(1),
                    std::future::pending,
                )),
                Err(crate::verification_owner::StartError::Operation(
                    crate::operations::ObservationFailure::AwaitingActor
                ))
            ));
            assert!(matches!(
                VerificationStart::resume(&operations, context.clone()),
                Err(crate::verification_owner::StartError::Operation(
                    crate::operations::ObservationFailure::AwaitingActor
                ))
            ));
            assert_no_packet(&sockets.worker);
        } else {
            assert!(matches!(
                futures_executor::block_on(starter.as_mut().unwrap().start(
                    &start_ports,
                    transport,
                    Duration::from_secs(1),
                    std::future::pending,
                )),
                Err(crate::verification_owner::StartError::Preparation(
                    crate::operations::VerificationPreparationFailure::Authority(
                        AuthorityRefusal::Revoked
                    )
                ))
            ));
            assert_no_packet(&sockets.worker);
            assert!(operations.has_retained_lease(operation));
            let allow = ClientAuthority {
                caller: caller.clone(),
                subject,
                boot,
                refusal: None,
            };
            let retry_ports = crate::verification_owner::VerificationStartPorts {
                registry: &operations,
                effects: &effects,
                authority: &allow,
                clock: clock.as_ref(),
                broker: &Cleanup,
                scheduler: &scheduler,
            };
            let retry = carosign_linux::sys::socketpair_seqpacket().unwrap();
            let transport = ProviderTransport::new(retry.platform, Channel::new()).unwrap();
            assert!(matches!(
                futures_executor::block_on(starter.as_mut().unwrap().start(
                    &retry_ports,
                    transport,
                    Duration::from_secs(1),
                    std::future::pending,
                )),
                Err(crate::verification_owner::StartError::Finished)
            ));
            assert_no_packet(&retry.worker);
        }
        return;
    }
    if matches!(
        dispatch,
        DispatchCase::Disconnected | DispatchCase::Dropped | DispatchCase::InvalidFrame
    ) {
        let mut attempt =
            futures_executor::block_on(operations.begin_revalidated_verification_send(
                &context,
                clock.as_ref(),
                &dispatch_authority,
            ))
            .unwrap();
        assert!(matches!(
            futures_executor::block_on(operations.begin_revalidated_verification_send(
                &context,
                clock.as_ref(),
                &dispatch_authority,
            )),
            Err(
                crate::operations::VerificationPreparationFailure::Operation(
                    crate::operations::ObservationFailure::ContextMismatch
                )
            )
        ));
        if matches!(dispatch, DispatchCase::InvalidFrame) {
            let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
            attempt.invalidate_frame_for_fixture();
            let transport = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
            assert!(matches!(
                attempt.send(transport, Duration::from_secs(1)),
                Err(crate::provider_verification::VerificationError::Protocol)
            ));
            let mut byte = [0u8; 1];
            // SAFETY: The socket and buffer remain valid for the nonblocking read.
            assert_eq!(
                unsafe {
                    libc::recv(
                        sockets.worker.as_raw_fd(),
                        byte.as_mut_ptr().cast(),
                        byte.len(),
                        libc::MSG_DONTWAIT,
                    )
                },
                0
            );
        } else if matches!(dispatch, DispatchCase::Disconnected) {
            let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
            // SAFETY: The test owns the descriptor and keeps it valid until closing it.
            assert_eq!(
                unsafe { libc::shutdown(sockets.worker.as_raw_fd(), libc::SHUT_RDWR) },
                0
            );
            drop(sockets.worker);
            let transport = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
            assert!(attempt.send(transport, Duration::from_secs(1)).is_err());
        } else {
            drop(attempt);
        }
        let delivery = VerificationContext {
            operation,
            worker: context.worker,
            command: context.command,
            authenticator: context.authenticator.id,
            epoch: context.authenticator.device_epoch,
        };
        let foreign = VerificationContext {
            command: CommandId::from_bytes([99; 16]).unwrap(),
            ..delivery
        };
        assert!(matches!(
            operations.deliver_verification_send_failure(&foreign),
            Err(crate::operations::ObservationFailure::ContextMismatch)
        ));
        let guard = operations.checkout(operation).unwrap();
        assert!(matches!(
            operations.deliver_verification_send_failure(&delivery),
            Err(crate::operations::ObservationFailure::Busy)
        ));
        drop(guard);
        let step = operations
            .deliver_verification_send_failure(&delivery)
            .unwrap();
        if matches!(dispatch, DispatchCase::InvalidFrame) {
            assert!(matches!(&step, Step::Advanced { state, effects }
                if state.common.phase == OperationPhase::Finalizing
                && effects.iter().any(|effect| effect.spec == carosign_engine::EffectSpec::ReleaseResources)));
        } else {
            assert!(matches!(&step, Step::Advanced { state, effects }
            if state.common.phase == OperationPhase::Cancelling
            && effects.iter().any(|effect| effect.spec == carosign_engine::EffectSpec::QuarantineScope)
            && !state.common.pending_effects.iter().any(|effect| effect.spec == carosign_engine::EffectSpec::SendVerify)));
        }
        assert!(operations.has_retained_lease(operation));
        assert!(
            operations
                .deliver_verification_send_failure(&delivery)
                .is_err()
        );
        let blocked = EffectOwner::default();
        blocked.accept_step(operation, &step).unwrap();
        assert!(matches!(
            operations.handoff_verification_send_failure(operation, &blocked),
            Err(crate::effect_owner::EffectOwnerError::Duplicate)
        ));
        operations
            .handoff_verification_send_failure(operation, &effects)
            .unwrap();
        assert!(matches!(
            effects.step(operation),
            Some(Step::Advanced { .. })
        ));
        assert!(
            operations
                .handoff_verification_send_failure(operation, &effects)
                .is_err()
        );
        return;
    }
    let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let process_binary = std::env::var_os("CAROSIGN_MOCK_PROVIDER_TEST_BINARY");
    assert!(
        std::env::var_os("CAROSIGN_SANDBOX_MOCK_HELPER").is_none() || process_binary.is_some(),
        "для CAROSIGN_SANDBOX_MOCK_HELPER требуется CAROSIGN_MOCK_PROVIDER_TEST_BINARY"
    );
    let (transport, worker, mut process) = if let Some(binary) = process_binary {
        let (transport, child) = prepare_mock_process(std::path::Path::new(&binary), sockets);
        (transport, None, Some(child))
    } else {
        let worker = std::thread::spawn(move || {
            let mut transport = ProviderTransport::new(sockets.worker, Channel::new()).unwrap();
            let request = transport.receive(Duration::from_secs(2)).unwrap();
            assert!(matches!(
                request.payload,
                Some(Payload::StartVerification(ref start))
                    if start.purpose == carosign_protocol::carosign::common::v1::VerificationPurpose::PurposeSessionUnlock as i32
            ));
            let response = |payload| provider::ProviderFrame {
                major: request.major,
                minor: request.minor,
                worker_instance_id: request.worker_instance_id.clone(),
                frame_seq: 0,
                command_id: request.command_id.clone(),
                operation_id: request.operation_id.clone(),
                device_epoch: request.device_epoch,
                uid: None,
                payload: Some(payload),
            };
            transport
                .send(
                    response(Payload::CommandAck(provider::CommandAck {
                        accepted: true,
                        rejection: None,
                    })),
                    Duration::from_secs(2),
                )
                .unwrap();
            transport
                .send(
                    response(Payload::VerificationOutcome(
                        provider::VerificationOutcome {
                            status: provider::VerificationStatus::VerifyNoUsableSample as i32,
                            ..Default::default()
                        },
                    )),
                    Duration::from_secs(2),
                )
                .unwrap();
        });
        let transport = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
        (transport, Some(worker), None)
    };
    let mut stale_start = None;
    if matches!(dispatch, DispatchCase::BusyStartOwner) {
        let busy = EffectOwner::default();
        let busy_ports = crate::verification_owner::VerificationStartPorts {
            registry: &operations,
            effects: &busy,
            authority: &dispatch_authority,
            clock: clock.as_ref(),
            broker: &Cleanup,
            scheduler: &scheduler,
        };
        busy.accept_step(operation, starter.as_ref().unwrap().pending_step().unwrap())
            .unwrap();
        let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let spare = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
        assert!(matches!(
            futures_executor::block_on(starter.as_mut().unwrap().start(
                &busy_ports,
                spare,
                Duration::from_secs(2),
                std::future::pending,
            )),
            Err(crate::verification_owner::StartError::Handoff(_))
        ));
        assert!(starter.as_ref().unwrap().pending_step().is_some());
        drop(starter.take());
        let mut foreign = context.clone();
        foreign.command = CommandId::from_bytes([99; 16]).unwrap();
        assert!(matches!(
            VerificationStart::resume(&operations, foreign),
            Err(crate::verification_owner::StartError::Operation(
                crate::operations::ObservationFailure::ContextMismatch
            ))
        ));
        starter = Some(VerificationStart::resume(&operations, context.clone()).unwrap());
        stale_start = Some(VerificationStart::resume(&operations, context.clone()).unwrap());
        assert_no_packet(&sockets.worker);
    }
    let mut owner = if let Some(start) = starter.as_mut() {
        futures_executor::block_on(start.start(
            &start_ports,
            transport,
            Duration::from_secs(2),
            std::future::pending,
        ))
        .unwrap()
    } else {
        let exchange = futures_executor::block_on(operations.begin_revalidated_verification_send(
            &context,
            clock.as_ref(),
            &dispatch_authority,
        ))
        .unwrap()
        .send(transport, Duration::from_secs(2))
        .unwrap();
        VerificationOwner::new(operation, VerificationPump::new(exchange))
    };
    if matches!(dispatch, DispatchCase::BusyStartOwner) {
        let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let spare = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
        assert!(matches!(
            futures_executor::block_on(starter.as_mut().unwrap().start(
                &start_ports,
                spare,
                Duration::from_secs(2),
                std::future::pending,
            )),
            Err(crate::verification_owner::StartError::Finished)
        ));
        assert_no_packet(&sockets.worker);
        let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let spare = ProviderTransport::new(sockets.platform, Channel::new()).unwrap();
        assert!(matches!(
            futures_executor::block_on(stale_start.as_mut().unwrap().start(
                &start_ports,
                spare,
                Duration::from_secs(2),
                std::future::pending,
            )),
            Err(crate::verification_owner::StartError::Operation(
                crate::operations::ObservationFailure::ContextMismatch
            ))
        ));
        assert_no_packet(&sockets.worker);
    }
    if uncertain {
        assert!(matches!(
            owner.next(&operations, Duration::from_secs(2)),
            Ok(OwnerEvent::StepReady)
        ));
        assert!(matches!(
            owner.next(&operations, Duration::from_secs(2)),
            Err(OwnerError::Pending)
        ));
        let busy_effects = EffectOwner::default();
        busy_effects
            .accept_step(operation, owner.pending_step().unwrap())
            .unwrap();
        assert!(matches!(
            owner.handoff(&busy_effects),
            Err(OwnerError::Handoff(_))
        ));
        assert!(matches!(
            owner.next(&operations, Duration::from_secs(2)),
            Err(OwnerError::Pending)
        ));
        assert!(matches!(
            futures_executor::block_on(owner.advance(
                &operations,
                &effects,
                &scheduler,
                &Cleanup,
                Duration::from_secs(2),
                std::future::pending
            )),
            Ok(AdvanceOutcome::Driven(
                crate::effect_owner::DriveOutcome::ExternalEffect
            ))
        ));
    }

    // The observer continues to work as long as the mediator holds the actor.
    use uncertain_cleanup::UncertainCleanup;
    let failed = UncertainCleanup {
        control: &control,
        operation,
        sender: watched
            .as_ref()
            .and_then(|(_, sender)| revoke.then_some(sender)),
        clock: expire.then_some(clock.as_ref()),
    };
    let broker: &dyn crate::resources::TrustedCleanupBroker =
        if uncertain { &failed } else { &Cleanup };
    let outcome = if uncertain {
        futures_executor::block_on(owner.advance(
            &operations,
            &effects,
            &scheduler,
            broker,
            Duration::from_secs(2),
            std::future::pending,
        ))
    } else {
        futures_executor::block_on(owner.drive_until_blocked(
            &operations,
            &effects,
            &scheduler,
            broker,
            Duration::from_secs(2),
            std::future::pending,
        ))
    };
    if (revoke || expire)
        && let Some((handle, _sender)) = watched.take()
    {
        handle.wait();
        assert!(matches!(
            control.claim(operation).unwrap().result(),
            crate::control_watch::WatchResult::Deferred
        ));
    }
    drop(owner);
    if let Some(worker) = worker {
        worker.join().unwrap();
    }
    if let Some(child) = process.as_mut() {
        assert!(
            child
                .wait_bounded(Duration::from_secs(5))
                .unwrap()
                .success()
        );
    }
    if uncertain {
        assert!(outcome.is_ok());
        assert!(operations.has_retained_lease(operation));
        assert_eq!(
            scheduler
                .lock()
                .unwrap()
                .fence_stage(Ids.new_lease_id().unwrap()),
            Some(carosign_runtime::scheduler::FenceStage::Uncertain)
        );
        assert!(
            futures_executor::block_on(service.prepare_completed_verification(&caller, operation))
                .is_err()
        );
        return;
    }
    assert!(matches!(
        outcome,
        Ok(AdvanceOutcome::Driven(
            crate::effect_owner::DriveOutcome::TerminalPending
        ))
    ));
    assert!(
        matches!(effects.step(operation), Some(Step::Advanced { ref state, .. }) if state.common.phase == OperationPhase::Terminal && state.common.terminal == Some(TerminalOutcome::NoSample))
    );
    let mut delivery =
        futures_executor::block_on(service.prepare_negative_delivery(&caller, operation)).unwrap();
    if matches!(dispatch, DispatchCase::LostPartialDelivery) {
        assert_eq!(
            futures_executor::block_on(delivery.try_send_one(
                &operations,
                &connection,
                &authority,
                clock.as_ref()
            )),
            Ok(crate::operations::OwnerSendProgress::Advanced),
        );
        drop(delivery);
        delivery = operations
            .reclaim_owner_delivery(operation, &caller)
            .unwrap();
    }
    if matches!(dispatch, DispatchCase::LostDelivery) {
        drop(delivery);
        let foreign = CallerContext::from_transport(
            [98; 32],
            uid,
            TransactionId::from_bytes([98; 16]).unwrap(),
        );
        assert!(matches!(
            operations.reclaim_owner_delivery(operation, &foreign),
            Err(crate::operations::OwnerDeliveryFailure::WrongOwner)
        ));
        delivery = operations
            .reclaim_owner_delivery(operation, &caller)
            .unwrap();
        assert!(matches!(
            operations.reclaim_owner_delivery(operation, &caller),
            Err(crate::operations::OwnerDeliveryFailure::AlreadyReserved)
        ));
    }
    if matches!(dispatch, DispatchCase::RevokedAfterDeliveryLoss) {
        drop(delivery);
        let _ = operations.revoke_verification(operation);
        assert!(matches!(
            operations.reclaim_owner_delivery(operation, &caller),
            Err(crate::operations::OwnerDeliveryFailure::Authority(
                AuthorityRefusal::Revoked
            ))
        ));
        let mut byte = [0u8; 1];
        // SAFETY: The socket and buffer remain valid for the nonblocking read.
        assert_eq!(
            unsafe {
                libc::recv(
                    client.worker.as_raw_fd(),
                    byte.as_mut_ptr().cast(),
                    1,
                    libc::MSG_DONTWAIT,
                )
            },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EAGAIN)
        );
        return;
    }
    if matches!(dispatch, DispatchCase::RevokeDuringDeliveryPolicy) {
        let revoking_policy = RevokeDuringDelivery {
            operations: &operations,
            operation,
        };
        assert!(matches!(
            futures_executor::block_on(delivery.try_send(
                &operations,
                &connection,
                &revoking_policy,
                clock.as_ref()
            )),
            Err(crate::operations::OwnerSendError::Delivery(
                crate::operations::OwnerDeliveryFailure::Authority(AuthorityRefusal::Revoked)
            ))
        ));
        let mut byte = [0u8; 1];
        // SAFETY: The socket and buffer remain valid for the nonblocking read.
        assert_eq!(
            unsafe {
                libc::recv(
                    client.worker.as_raw_fd(),
                    byte.as_mut_ptr().cast(),
                    1,
                    libc::MSG_DONTWAIT,
                )
            },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EAGAIN)
        );
        return;
    }
    if matches!(dispatch, DispatchCase::ExpireBeforeDelivery) {
        clock.advance(61_000_000_000);
        assert!(matches!(
            futures_executor::block_on(delivery.try_send(
                &operations,
                &connection,
                &authority,
                clock.as_ref()
            )),
            Err(crate::operations::OwnerSendError::Delivery(
                crate::operations::OwnerDeliveryFailure::Operation(
                    crate::operations::ObservationFailure::DeadlineExpired
                )
            ))
        ));
        let mut byte = [0u8; 1];
        // SAFETY: The socket and buffer remain valid for the nonblocking read.
        assert_eq!(
            unsafe {
                libc::recv(
                    client.worker.as_raw_fd(),
                    byte.as_mut_ptr().cast(),
                    1,
                    libc::MSG_DONTWAIT,
                )
            },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EAGAIN)
        );
        return;
    }
    if matches!(dispatch, DispatchCase::RevokeBeforeDelivery) {
        let _ = operations.revoke_verification(operation);
        assert!(matches!(
            futures_executor::block_on(delivery.try_send(
                &operations,
                &connection,
                &authority,
                clock.as_ref()
            )),
            Err(crate::operations::OwnerSendError::Delivery(
                crate::operations::OwnerDeliveryFailure::Authority(AuthorityRefusal::Revoked)
            ))
        ));
        let mut byte = [0u8; 1];
        // SAFETY: The socket and buffer remain valid for the nonblocking read.
        let count = unsafe {
            libc::recv(
                client.worker.as_raw_fd(),
                byte.as_mut_ptr().cast(),
                1,
                libc::MSG_DONTWAIT,
            )
        };
        assert_eq!(count, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EAGAIN)
        );
        return;
    }
    if matches!(
        dispatch,
        DispatchCase::FullDeliverySocket
            | DispatchCase::RevokedWhileDeliveryBlocked
            | DispatchCase::PolicyRevokedWhileDeliveryBlocked
            | DispatchCase::LostBlockedDelivery
    ) {
        let fill = [0u8; 1024];
        let mut full = false;
        for _ in 0..16_384 {
            // SAFETY: The socket and buffer remain valid for the nonblocking send.
            let n = unsafe {
                libc::send(
                    connection.fd.as_raw_fd(),
                    fill.as_ptr().cast(),
                    fill.len(),
                    libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                )
            };
            if n == -1 {
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::EAGAIN)
                );
                full = true;
                break;
            }
            assert_eq!(n, fill.len() as isize);
        }
        assert!(full, "буфер сокета не заполнился");
        assert_eq!(
            futures_executor::block_on(delivery.try_send(
                &operations,
                &connection,
                &authority,
                clock.as_ref()
            )),
            Ok(crate::operations::OwnerSendProgress::WouldBlock)
        );
        if matches!(dispatch, DispatchCase::LostBlockedDelivery) {
            drop(delivery);
            delivery = operations
                .reclaim_owner_delivery(operation, &caller)
                .unwrap();
        }
        if matches!(dispatch, DispatchCase::RevokedWhileDeliveryBlocked) {
            let _ = operations.revoke_verification(operation);
        }
        let mut drain = [0u8; 1024];
        loop {
            // SAFETY: The socket and buffer remain valid for the nonblocking read.
            let n = unsafe {
                libc::recv(
                    client.worker.as_raw_fd(),
                    drain.as_mut_ptr().cast(),
                    drain.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            if n == -1 {
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::EAGAIN)
                );
                break;
            }
            assert_eq!(n, fill.len() as isize);
        }
        if matches!(dispatch, DispatchCase::RevokedWhileDeliveryBlocked) {
            assert!(matches!(
                futures_executor::block_on(delivery.try_send(
                    &operations,
                    &connection,
                    &authority,
                    clock.as_ref()
                )),
                Err(crate::operations::OwnerSendError::Delivery(
                    crate::operations::OwnerDeliveryFailure::Authority(AuthorityRefusal::Revoked)
                ))
            ));
            let mut byte = [0u8; 1];
            // SAFETY: The socket and buffer remain valid for the nonblocking read.
            assert_eq!(
                unsafe {
                    libc::recv(
                        client.worker.as_raw_fd(),
                        byte.as_mut_ptr().cast(),
                        1,
                        libc::MSG_DONTWAIT,
                    )
                },
                -1
            );
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EAGAIN)
            );
            return;
        }
        if matches!(dispatch, DispatchCase::PolicyRevokedWhileDeliveryBlocked) {
            let denied = ClientAuthority {
                caller: caller.clone(),
                subject,
                boot,
                refusal: Some(AuthorityRefusal::Revoked),
            };
            assert!(matches!(
                futures_executor::block_on(delivery.try_send(
                    &operations,
                    &connection,
                    &denied,
                    clock.as_ref()
                )),
                Err(crate::operations::OwnerSendError::Delivery(
                    crate::operations::OwnerDeliveryFailure::Authority(AuthorityRefusal::Revoked)
                ))
            ));
            assert!(matches!(
                futures_executor::block_on(delivery.try_send(
                    &operations,
                    &connection,
                    &authority,
                    clock.as_ref()
                )),
                Err(crate::operations::OwnerSendError::Finished)
            ));
            let mut byte = [0u8; 1];
            // SAFETY: The socket and buffer remain valid for the nonblocking read.
            assert_eq!(
                unsafe {
                    libc::recv(
                        client.worker.as_raw_fd(),
                        byte.as_mut_ptr().cast(),
                        1,
                        libc::MSG_DONTWAIT,
                    )
                },
                -1
            );
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EAGAIN)
            );
            return;
        }
    }
    assert_eq!(
        futures_executor::block_on(delivery.try_send(
            &operations,
            &connection,
            &authority,
            clock.as_ref()
        )),
        Ok(crate::operations::OwnerSendProgress::Sent),
    );
    assert!(matches!(
        futures_executor::block_on(delivery.try_send(
            &operations,
            &connection,
            &authority,
            clock.as_ref()
        )),
        Err(crate::operations::OwnerSendError::Finished)
    ));
    drop(delivery);
    assert!(matches!(
        operations.reclaim_owner_delivery(operation, &caller),
        Err(crate::operations::OwnerDeliveryFailure::AlreadyReserved)
    ));
    read_no_sample_until_terminal(client.worker.as_raw_fd());
}

fn read_no_sample_until_terminal(fd: std::os::fd::RawFd) {
    use carosign_protocol::carosign::client::v1 as client;
    use prost::Message;
    let mut expected = 1;
    loop {
        let mut buffer = vec![0; 65536];
        // SAFETY: The descriptor and buffer are valid until the end of the reading.
        let size = unsafe { libc::recv(fd, buffer.as_mut_ptr().cast(), buffer.len(), 0) };
        assert!(size > 0);
        let frame = client::ClientFrame::decode(&buffer[..size as usize]).unwrap();
        let Some(client::client_frame::Payload::OperationEvent(event)) = frame.payload else {
            panic!("ожидалось событие операции");
        };
        assert_eq!(event.sequence, expected);
        assert_eq!(frame.event_sequence, expected);
        expected += 1;
        if let Some(terminal) = event.terminal {
            assert!(
                matches!(terminal.kind, Some(client::terminal_result::Kind::Verification(result))
                if result.result == client::VerificationResult::VerifyNoSample as i32)
            );
            break;
        }
    }
}

#[test]
fn counted_public_conversation_drives_mock_verification() {
    exercise_service_cleanup(false, false, false, DispatchCase::PublicLoop);
}

fn connect_public_client(path: &str) -> std::os::fd::OwnedFd {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    // SAFETY: A successful socket transfers a new descriptor to a single owner.
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    assert!(raw >= 0);
    let socket = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: zero-point sockaddr_un Then all the fields used are filled.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    assert!(path.len() < address.sun_path.len());
    for (dst, src) in address.sun_path.iter_mut().zip(path.as_bytes()) {
        *dst = *src as libc::c_char;
    }
    let length =
        (std::mem::offset_of!(libc::sockaddr_un, sun_path) + path.len() + 1) as libc::socklen_t;
    // SAFETY: The address and socket are valid during the call.
    assert_eq!(
        unsafe {
            libc::connect(
                socket.as_raw_fd(),
                (&address as *const libc::sockaddr_un).cast(),
                length,
            )
        },
        0
    );
    socket
}

/// Test Bridge: Caller trust and device metadata are only substituted in this test module.
fn run_fprintd_client(
    transport: carosign_linux_ipc::client::transport::Transport,
    request: carosign_protocol::carosign::client::v1::BeginVerification,
    cancel_test: Option<std::sync::mpsc::Receiver<()>>,
) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, mpsc};
    use std::time::{Duration, Instant};

    use test_bridge::TestBridge;

    let saw_no_sample = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&saw_no_sample);
    let done = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&done);
    let (ready_tx, ready_rx) = mpsc::channel();
    let service = std::thread::spawn(move || {
        let connection = dbus::blocking::Connection::new_session().unwrap();
        let mut backend = TestBridge {
            transport: Arc::new(transport),
            request: Some(request),
            saw_no_sample: observed,
            pending: None,
            cancellation: None,
        };
        ready_tx.send(()).unwrap();
        carosign_fprintd_compat::dbus::serve_with_backend(&connection, &mut backend, || {
            stop.load(Ordering::SeqCst)
        })
        .unwrap();
    });
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let client = dbus::blocking::Connection::new_session().unwrap();
    let manager = client.with_proxy(
        "net.reactivated.Fprint",
        "/net/reactivated/Fprint/Manager",
        Duration::from_secs(1),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result: Result<(dbus::Path<'static>,), _> =
            manager.method_call("net.reactivated.Fprint.Manager", "GetDefaultDevice", ());
        if result.is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "фасад не появился на испытательной шине"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    if let Some(accepted) = cancel_test {
        let (path,): (dbus::Path<'static>,) = manager
            .method_call("net.reactivated.Fprint.Manager", "GetDefaultDevice", ())
            .unwrap();
        let device = client.with_proxy("net.reactivated.Fprint", path, Duration::from_secs(3));
        let interface = "net.reactivated.Fprint.Device";
        device
            .method_call::<(), _, _, _>(interface, "Claim", ("",))
            .unwrap();
        device
            .method_call::<(), _, _, _>(interface, "VerifyStart", ("right-index-finger",))
            .unwrap();
        accepted.recv_timeout(Duration::from_secs(5)).unwrap();
        device
            .method_call::<(), _, _, _>(interface, "VerifyStop", ())
            .unwrap();
        device
            .method_call::<(), _, _, _>(interface, "Release", ())
            .unwrap();
        done.store(true, Ordering::SeqCst);
        service.join().unwrap();
        return;
    }
    let output = std::process::Command::new("fprintd-verify")
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
        )
        .output()
        .expect("нужен установленный fprintd-verify");
    done.store(true, Ordering::SeqCst);
    service.join().unwrap();
    assert!(
        saw_no_sample.load(Ordering::SeqCst),
        "запрос не дошёл до службы и мок-провайдера: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("verify-unknown-error"),
        "неверный результат: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The test client receives only the client socket and the synchronization channel.
fn spawn_flow_client_process(
    client: std::os::fd::OwnedFd,
    matched: bool,
) -> (std::process::Child, std::os::unix::net::UnixStream) {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    let (parent, child_sync) = std::os::unix::net::UnixStream::pair().unwrap();
    // SAFETY: F_DUPFD Create independent descriptors without CLOEXEC.
    let client_raw = unsafe { libc::fcntl(client.as_raw_fd(), libc::F_DUPFD, 3) };
    let sync_raw = unsafe { libc::fcntl(child_sync.as_raw_fd(), libc::F_DUPFD, 3) };
    assert!(client_raw >= 0 && sync_raw >= 0);
    // SAFETY: Each successful `F_DUPFD` call returns a new descriptor owned by this test.
    let inherited_client = unsafe { OwnedFd::from_raw_fd(client_raw) };
    let inherited_sync = unsafe { OwnedFd::from_raw_fd(sync_raw) };
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "service_completion_tests::tunnel_flow_client_process_fixture",
            "--ignored",
            "--nocapture",
        ])
        .env("CAROSIGN_FLOW_CLIENT_FD", client_raw.to_string())
        .env("CAROSIGN_FLOW_CLIENT_SYNC_FD", sync_raw.to_string())
        .env(
            "CAROSIGN_FLOW_CLIENT_MATCHED",
            if matched { "1" } else { "0" },
        )
        .env("CAROSIGN_TEST_PAM_OBSERVER", "1")
        .spawn()
        .unwrap();
    drop((inherited_client, inherited_sync, child_sync, client));
    (child, parent)
}

#[test]
#[ignore = "дочерний процесс клиента испытательного tunnel-состава"]
fn tunnel_flow_client_process_fixture() {
    use std::io::{Read, Write};
    use std::os::fd::{FromRawFd, OwnedFd};
    let fd: i32 = std::env::var("CAROSIGN_FLOW_CLIENT_FD")
        .unwrap()
        .parse()
        .unwrap();
    let sync_fd: i32 = std::env::var("CAROSIGN_FLOW_CLIENT_SYNC_FD")
        .unwrap()
        .parse()
        .unwrap();
    // SAFETY: The fixture parent transferred exclusive ownership through `F_DUPFD`.
    let client = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut sync = unsafe { std::os::unix::net::UnixStream::from_raw_fd(sync_fd) };
    if std::env::var("CAROSIGN_FLOW_CLIENT_MATCHED").unwrap() != "1" {
        run_public_client(client, AuthenticatorId::from_bytes([5; 16]).unwrap());
        return;
    }
    use carosign_protocol::carosign::client::v1 as wire;
    let transport =
        carosign_linux_ipc::client::transport::Transport::from_connected(client).unwrap();
    let request = wire::BeginVerification {
        report_matched_enrollment: false,
        enrollment: None,
        idempotency_key: vec![12; 16],
        authenticator_id: vec![5; 16],
        subject: Some(carosign_protocol::carosign::common::v1::SubjectSelector {
            selector: Some(
                carosign_protocol::carosign::common::v1::subject_selector::Selector::CurrentSubject(
                    true,
                ),
            ),
        }),
        purpose_hint:
            carosign_protocol::carosign::common::v1::VerificationPurpose::PurposeSessionLogin as i32,
        authority_request_ref: vec![9, 8],
    };
    let result =
        pam_carosign::client::observe_verification_with_acceptance(&transport, request, |id| {
            sync.write_all(id).unwrap();
            let mut resume = [0];
            sync.read_exact(&mut resume).unwrap();
            assert_eq!(resume, [1]);
        })
        .unwrap();
    assert_eq!(result, pam_carosign::client::Observation::FactorAccepted);
}

fn run_public_client(client_fd: std::os::fd::OwnedFd, authenticator: AuthenticatorId) {
    use carosign_protocol::carosign::client::v1 as client;
    let transport =
        carosign_linux_ipc::client::transport::Transport::from_connected(client_fd).unwrap();
    let request = client::BeginVerification {
        report_matched_enrollment: false,
        enrollment: None,
        idempotency_key: vec![12; 16],
        authenticator_id: authenticator.as_bytes().to_vec(),
        subject: Some(carosign_protocol::carosign::common::v1::SubjectSelector {
            selector: Some(
                carosign_protocol::carosign::common::v1::subject_selector::Selector::CurrentSubject(
                    true,
                ),
            ),
        }),
        purpose_hint:
            carosign_protocol::carosign::common::v1::VerificationPurpose::PurposeSessionLogin as i32,
        authority_request_ref: vec![9, 8],
    };
    if std::env::var_os("CAROSIGN_TEST_FPRINTD_ADAPTER").is_some() {
        run_fprintd_client(transport, request, None);
        return;
    }
    if std::env::var_os("CAROSIGN_TEST_PAM_OBSERVER").is_some() {
        // This is a protocol check, not a PAM authorization or proof of trust in a test
        // source of authority.
        let result = pam_carosign::client::observe_verification(&transport, request).unwrap();
        assert_eq!(result, pam_carosign::client::Observation::NoSample);
        return;
    }
    let response = transport
        .request(client::client_frame::Payload::BeginVerification(request))
        .unwrap();
    let Some(client::client_frame::Payload::OperationAccepted(accepted)) = response.payload else {
        panic!("операция не принята");
    };
    let mut tracker =
        carosign_linux_ipc::client::verification_events::VerificationEvents::new(&accepted)
            .unwrap();
    loop {
        let (frame, terminal) = transport.receive_verification_event(&mut tracker).unwrap();
        if terminal {
            assert!(
                matches!(frame.payload, Some(client::client_frame::Payload::OperationEvent(event))
                if matches!(event.terminal.as_ref().and_then(|result| result.kind.as_ref()),
                    Some(client::terminal_result::Kind::Verification(result))
                    if result.result == client::VerificationResult::VerifyNoSample as i32))
            );
            break;
        }
    }
}

/// D-Bus Test → shared transport with the individual socket test owner. The test response is
/// not an endorsement of service authority.
#[test]
#[ignore = "требует отдельную сессионную D-Bus-шину"]
fn fprintd_verify_stop_sends_cancel_to_socket_peer() {
    use carosign_protocol::carosign::{client::v1 as wire, common::v1 as common};
    use prost::Message;
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::sync::mpsc;
    use wire::client_frame::Payload;

    if let Some(raw) = std::env::var_os("CAROSIGN_TEST_CANCEL_PEER_FD") {
        let fd: i32 = raw.to_str().unwrap().parse().unwrap();
        let signal_fd: i32 = std::env::var("CAROSIGN_TEST_CANCEL_SIGNAL_FD")
            .unwrap()
            .parse()
            .unwrap();
        // SAFETY: the parent transferred exclusive ownership of the descriptors to the child
        // process.
        let _socket = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut signal = unsafe { std::fs::File::from_raw_fd(signal_fd) };
        let read = || {
            let mut bytes = [0u8; 65536];
            // SAFETY: Socket and buffer are valid until the end of the reception.
            let n = unsafe { libc::recv(fd, bytes.as_mut_ptr().cast(), bytes.len(), 0) };
            assert!(n > 0);
            wire::ClientFrame::decode(&bytes[..n as usize]).unwrap()
        };
        let send = |frame: wire::ClientFrame| {
            let bytes = frame.encode_to_vec();
            // SAFETY: The socket and buffer are valid until the end of the shipment.
            assert_eq!(
                unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), libc::MSG_NOSIGNAL) },
                bytes.len() as isize
            );
        };
        let hello = read();
        let Some(Payload::Hello(body)) = hello.payload else {
            panic!("ожидался Hello")
        };
        let minor = body.max_minor;
        send(wire::ClientFrame {
            major: 1,
            minor,
            request_id: hello.request_id,
            operation_id: vec![],
            event_sequence: 0,
            payload: Some(Payload::HelloAck(wire::HelloAck {
                verification_enrollment_selector: false,
                capture_feedback_v1: false,
                major: 1,
                selected_minor: minor,
                connection_id: vec![7; 16],
            })),
        });
        let begin = read();
        assert!(matches!(begin.payload, Some(Payload::BeginVerification(_))));
        let operation = vec![8; 16];
        send(wire::ClientFrame {
            major: 1,
            minor,
            request_id: begin.request_id,
            operation_id: operation.clone(),
            event_sequence: 0,
            payload: Some(Payload::OperationAccepted(wire::OperationAccepted {
                operation_id: operation.clone(),
                first_event_sequence: 1,
                deadline: Some(common::BootDeadline {
                    boot_id: vec![9; 16],
                    boottime_ns: 1000,
                }),
            })),
        });
        signal.write_all(&[1]).unwrap();
        let cancel = read();
        assert!(
            matches!(&cancel.payload, Some(Payload::CancelOperation(value)) if value.operation_id == operation)
        );
        send(wire::ClientFrame {
            major: 1,
            minor,
            request_id: cancel.request_id,
            operation_id: vec![],
            event_sequence: 0,
            payload: Some(Payload::CancellationReceipt(wire::CancellationReceipt {
                result: wire::CancelReceiptKind::CancelReceiptRequested as i32,
            })),
        });
        return;
    }
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let (mut signal_reader, signal_writer) = std::os::unix::net::UnixStream::pair().unwrap();
    // SAFETY: F_DUPFD Creates separate descriptors without CLOEXEC for the child process.
    let peer_raw = unsafe { libc::fcntl(pair.worker.as_raw_fd(), libc::F_DUPFD, 3) };
    let signal_raw = unsafe { libc::fcntl(signal_writer.as_raw_fd(), libc::F_DUPFD, 3) };
    assert!(peer_raw >= 0 && signal_raw >= 0);
    let peer = unsafe { OwnedFd::from_raw_fd(peer_raw) };
    let signal = unsafe { OwnedFd::from_raw_fd(signal_raw) };
    let mut server = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "service_completion_tests::fprintd_verify_stop_sends_cancel_to_socket_peer",
            "--ignored",
            "--nocapture",
        ])
        .env("CAROSIGN_TEST_CANCEL_PEER_FD", peer_raw.to_string())
        .env("CAROSIGN_TEST_CANCEL_SIGNAL_FD", signal_raw.to_string())
        .spawn()
        .unwrap();
    drop((peer, signal, signal_writer, pair.worker));
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let mut byte = [0u8; 1];
        signal_reader.read_exact(&mut byte).unwrap();
        accepted_tx.send(()).unwrap();
    });
    let transport =
        carosign_linux_ipc::client::transport::Transport::from_connected(pair.platform).unwrap();
    let request = wire::BeginVerification {
        report_matched_enrollment: false,
        enrollment: None,
        idempotency_key: vec![12; 16],
        authenticator_id: vec![7; 16],
        subject: Some(common::SubjectSelector {
            selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
        }),
        purpose_hint: common::VerificationPurpose::PurposeSessionLogin as i32,
        authority_request_ref: vec![9, 8],
    };
    run_fprintd_client(transport, request, Some(accepted_rx));
    waiter.join().unwrap();
    assert!(server.wait().unwrap().success());
}

#[test]
#[ignore = "дочерний процесс испытательного клиента"]
fn public_client_process_fixture() {
    let path = std::env::var("CAROSIGN_TEST_CLIENT_SOCKET").unwrap();
    let client = connect_public_client(&path);
    run_public_client(client, AuthenticatorId::from_bytes([7; 16]).unwrap());
}

/// It runs the test staff of the service and the client in separate processes, and the mock
/// must also be a separate process, as in an isolated run.
#[test]
#[ignore = "запускается сценарием test-isolated-mock.sh с отдельным моком"]
fn public_flow_in_separate_service_and_client_processes() {
    assert!(
        std::env::var_os("CAROSIGN_MOCK_PROVIDER_TEST_BINARY").is_some(),
        "для процессного испытания нужен отдельный мок"
    );
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "service_completion_tests::counted_public_conversation_drives_mock_verification",
            "--nocapture",
        ])
        .env("CAROSIGN_TEST_SEPARATE_CLIENT", "1")
        .spawn()
        .unwrap();
    let mut child = MockChild::new(child).unwrap();
    assert!(
        child
            .wait_bounded(std::time::Duration::from_secs(20))
            .unwrap()
            .success()
    );
}

#[allow(clippy::too_many_arguments)]
fn exercise_public_loop(
    listener: &std::os::fd::OwnedFd,
    service: &crate::service::Service<
        '_,
        MemoryStore,
        ClientAuthority,
        Ids,
        carosign_adapters::clock::StepClock,
    >,
    authority: &ClientAuthority,
    registry: std::sync::Arc<crate::operations::OperationRegistry>,
    clock: std::sync::Arc<carosign_adapters::clock::StepClock>,
    scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
    selection: (SubjectRef, AuthenticatorId),
    client_path: &str,
) {
    use crate::talk::Ended;

    use std::time::Duration;

    let (subject, authenticator) = selection;

    use dispatcher::Dispatcher;
    let registry_for_assert = registry.clone();
    let expected_pid = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(std::process::id()));
    let dispatcher = Dispatcher {
        service,
        authority,
        registry,
        clock,
        scheduler,
        subject,
        authenticator,
        expected_pid: expected_pid.clone(),
    };
    use client_run::ClientRun;
    let mut client_run = if std::env::var_os("CAROSIGN_TEST_SEPARATE_CLIENT").is_some() {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "service_completion_tests::public_client_process_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("CAROSIGN_TEST_CLIENT_SOCKET", client_path);
        let child = command.spawn().unwrap();
        expected_pid.store(child.id(), std::sync::atomic::Ordering::Release);
        ClientRun::Process(MockChild::new(child).unwrap())
    } else {
        let client_path = client_path.to_owned();
        ClientRun::Thread(std::thread::spawn(move || {
            run_public_client(connect_public_client(&client_path), authenticator)
        }))
    };
    let invalid = crate::config::Config {
        hello_timeout_s: 0,
        ..Default::default()
    };
    assert!(matches!(
        futures_executor::block_on(crate::talk::serve_counted_once_with_dispatcher(
            listener,
            service,
            registry_for_assert.clone(),
            crate::talk::Asked {
                subject,
                epoch: DeviceEpoch::FIRST
            },
            &crate::health::UnavailableHealth,
            &invalid,
            &dispatcher,
        )),
        Err(crate::talk::CountedServeError::InvalidConfig)
    ));
    let result = futures_executor::block_on(crate::talk::serve_counted_once_with_dispatcher(
        listener,
        service,
        registry_for_assert.clone(),
        crate::talk::Asked {
            subject,
            epoch: DeviceEpoch::FIRST,
        },
        &crate::health::UnavailableHealth,
        &crate::config::Config::default(),
        &dispatcher,
    ));
    match &mut client_run {
        ClientRun::Process(child) => assert!(
            child
                .wait_bounded(Duration::from_secs(10))
                .unwrap()
                .success()
        ),
        ClientRun::Thread(_) => {}
    }
    if let ClientRun::Thread(thread) = client_run {
        thread.join().unwrap();
    }
    assert!(matches!(result, Ok(Ended::Closed)));
    let slots: Vec<_> = (0..crate::operations::DEFAULT_CONNECTIONS)
        .map(|_| {
            registry_for_assert
                .open_connection(4)
                .expect("место соединения освобождено")
        })
        .collect();
    assert!(registry_for_assert.open_connection(4).is_none());
    for slot in slots {
        registry_for_assert.close_connection(slot);
    }
}

#[test]
fn verification_timeout_releases_resources_and_delivers_result() {
    exercise_service_cleanup(false, false, false, DispatchCase::FlowTimeout);
}

#[test]
fn admitted_dispatch_retains_foreign_lease_without_calling_provider() {
    exercise_service_cleanup(false, false, false, DispatchCase::ForeignLease);
}

#[test]
fn admitted_dispatch_retains_foreign_worker_lease_without_calling_provider() {
    exercise_service_cleanup(false, false, false, DispatchCase::ForeignWorker);
}

#[test]
fn revoked_after_preparation_never_reaches_client_socket() {
    exercise_service_cleanup(false, false, false, DispatchCase::RevokeBeforeDelivery);
}

#[test]
fn complete_flow_fences_shared_reader_until_confirmed_release() {
    exercise_service_cleanup(false, false, false, DispatchCase::FlowSharedCleanup);
}

#[test]
fn expired_after_preparation_never_reaches_client_socket() {
    exercise_service_cleanup(false, false, false, DispatchCase::ExpireBeforeDelivery);
}

#[test]
fn full_client_socket_keeps_result_until_one_successful_delivery() {
    exercise_service_cleanup(false, false, false, DispatchCase::FullDeliverySocket);
}

#[test]
fn reclaimed_delivery_resumes_after_already_sent_event() {
    exercise_service_cleanup(false, false, false, DispatchCase::LostPartialDelivery);
}

#[test]
fn abandoned_delivery_returns_to_registry_without_duplicate_owner() {
    exercise_service_cleanup(false, false, false, DispatchCase::LostDelivery);
}

#[test]
fn abandoned_blocked_delivery_can_be_reclaimed_once() {
    exercise_service_cleanup(false, false, false, DispatchCase::LostBlockedDelivery);
}

#[test]
fn revoked_abandoned_delivery_is_never_reclaimed() {
    exercise_service_cleanup(false, false, false, DispatchCase::RevokedAfterDeliveryLoss);
}

#[test]
fn revocation_while_client_socket_is_full_cancels_retained_delivery() {
    exercise_service_cleanup(
        false,
        false,
        false,
        DispatchCase::RevokedWhileDeliveryBlocked,
    );
}

#[test]
fn policy_is_revalidated_before_retry_of_retained_delivery() {
    exercise_service_cleanup(
        false,
        false,
        false,
        DispatchCase::PolicyRevokedWhileDeliveryBlocked,
    );
}

#[test]
fn revocation_during_policy_revalidation_is_checked_before_send() {
    exercise_service_cleanup(
        false,
        false,
        false,
        DispatchCase::RevokeDuringDeliveryPolicy,
    );
}

#[test]
fn admitted_dispatch_retains_lease_when_revoked_during_acquire() {
    exercise_service_cleanup(false, false, false, DispatchCase::RevokeDuringAcquire);
}

#[test]
fn admitted_dispatch_retains_lease_when_deadline_expires_during_acquire() {
    exercise_service_cleanup(false, false, false, DispatchCase::ExpireDuringAcquire);
}

#[test]
fn bound_start_retains_step_when_owner_is_busy_and_never_resends() {
    exercise_service_cleanup(false, false, false, DispatchCase::BusyStartOwner);
}

#[test]
fn bound_start_reports_uncertain_transport_without_repeating() {
    exercise_service_cleanup(false, false, false, DispatchCase::StartDisconnected);
}

#[test]
fn bound_start_revalidates_before_sending() {
    exercise_service_cleanup(false, false, false, DispatchCase::StartRevoked);
}

#[test]
fn bound_start_cannot_resume_or_send_after_control_revocation() {
    exercise_service_cleanup(false, false, false, DispatchCase::StartControlRevoked);
}

#[test]
fn complete_flow_retains_delivery_on_full_socket() {
    exercise_service_cleanup(false, false, false, DispatchCase::FlowBlocked);
}

#[test]
fn complete_flow_rejects_revocation_before_dispatch() {
    exercise_service_cleanup(false, false, false, DispatchCase::FlowRevoked);
}

#[test]
fn complete_flow_retains_uncertain_dispatch_on_disconnect() {
    exercise_service_cleanup(false, false, false, DispatchCase::FlowDisconnected);
}

#[test]
fn service_operation_reaches_client_after_mock_provider_and_confirmed_mock_cleanup() {
    exercise_service_cleanup(false, false, false, DispatchCase::Normal);
}

#[test]
#[ignore = "требуются отдельные испытательные broker и mock-провайдер"]
fn tunnel_broker_mock_reaches_client_after_confirmed_cleanup() {
    assert!(std::env::var_os("CAROSIGN_FLOW_TEST_TUNNEL").is_some());
    exercise_service_cleanup(false, false, false, DispatchCase::Normal);
}

#[test]
#[ignore = "требуются отдельные испытательные broker и mock-провайдер"]
fn tunnel_broker_matched_reaches_client_after_independent_verification() {
    assert!(std::env::var_os("CAROSIGN_FLOW_TEST_TUNNEL").is_some());
    exercise_service_cleanup(false, false, false, DispatchCase::FlowMatched);
}

#[test]
#[ignore = "требует отдельный mock-провайдер в CAROSIGN_MOCK_PROVIDER_TEST_BINARY"]
fn matched_mock_reaches_client_after_independent_verification_and_cleanup() {
    exercise_service_cleanup(false, false, false, DispatchCase::FlowMatched);
}

#[test]
#[ignore = "требует отдельный mock-провайдер в CAROSIGN_MOCK_PROVIDER_TEST_BINARY"]
fn scoped_mock_verified_assertion_reaches_client_after_cleanup() {
    exercise_service_cleanup(false, false, false, DispatchCase::FlowScoped);
}

#[test]
#[ignore = "требуются отдельные испытательные broker и mock-провайдер"]
fn tunnel_broker_scoped_assertion_reaches_client_after_cleanup() {
    assert!(std::env::var_os("CAROSIGN_FLOW_TEST_TUNNEL").is_some());
    assert!(std::env::var_os("CAROSIGN_FLOW_SEPARATE_CLIENT").is_some());
    exercise_service_cleanup(false, false, false, DispatchCase::FlowScoped);
}

#[test]
fn service_uncertain_cleanup_keeps_lease_and_blocks_client_result() {
    exercise_service_cleanup(true, false, false, DispatchCase::Normal);
}

#[test]
fn service_revocation_during_uncertain_cleanup_keeps_lease_and_blocks_result() {
    exercise_service_cleanup(true, true, false, DispatchCase::Normal);
}

#[test]
fn service_expiry_during_uncertain_cleanup_keeps_lease_and_blocks_result() {
    exercise_service_cleanup(true, false, true, DispatchCase::Normal);
}

#[test]
fn revoked_before_dispatch_refuses_frame_and_keeps_binding_step() {
    exercise_service_cleanup(false, false, false, DispatchCase::Revoked);
}

#[test]
fn expired_before_dispatch_refuses_frame_and_keeps_binding_step() {
    exercise_service_cleanup(false, false, false, DispatchCase::Expired);
}

#[test]
fn disconnected_send_is_retained_without_retry_until_actor_accepts_it() {
    exercise_service_cleanup(false, false, false, DispatchCase::Disconnected);
}

#[test]
fn abandoned_send_is_conservatively_retained_without_retry() {
    exercise_service_cleanup(false, false, false, DispatchCase::Dropped);
}

#[test]
fn invalid_frame_is_proven_unsent_and_requests_cleanup() {
    exercise_service_cleanup(false, false, false, DispatchCase::InvalidFrame);
}

#[test]
fn cancellation_during_send_never_releases_or_dispatches_after_revocation() {
    exercise_service_cleanup(false, false, false, DispatchCase::CancelDuringSend);
}

#[test]
fn cancellation_after_unknown_send_quarantines_instead_of_releasing() {
    exercise_service_cleanup(false, false, false, DispatchCase::CancelAfterUnknown);
}
