//! End-to-end verification of cancellation of this dispatcher through a separate mock.

use carosign_core::{
    enrollment::{EnrollmentGeneration, EnrollmentRef},
    identity::SubjectRef,
    ids::{AuthorityId, EnrollmentId},
};
use carosign_persistence::{
    enrollment_codec::{EnrollmentRecord, EnrollmentStatus},
    store::{MetadataStore, Table},
};
use carosign_protocol::carosign::{client::v1 as wire, common::v1 as common};
use std::sync::Arc;
use wire::client_frame::Payload;

#[test]
#[ignore = "требует broker и mock в сценарии linux-pending-mutation"]
fn mutation_cancel_keeps_scope_for_reconciliation() {
    cancel_enrollment(false);
}

#[path = "linux_dispatch.rollback.tests.rs"]
mod rollback_tests;

fn cancel_enrollment(expect_rollback: bool) {
    cancel_enrollment_via(expect_rollback, false);
}

fn cancel_enrollment_via(expect_rollback: bool, separate_connection: bool) {
    exercise_enrollment_rollback(expect_rollback, separate_connection, true);
}

fn exercise_enrollment_rollback(
    expect_rollback: bool,
    separate_connection: bool,
    send_cancel: bool,
) {
    let path =
        std::env::temp_dir().join(format!("carosign-mutation-cancel-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let policy_path = path.join("policy");
    let uid = unsafe { libc::getuid() };
    std::fs::write(
        &policy_path,
        format!("uid={uid} subject={uid} actions=enroll,cancel lifetime=30\n"),
    )
    .unwrap();
    let store = Arc::new(
        carosign_persistence::redb::backend::RedbStore::create_current(&path.join("store.redb"))
            .unwrap(),
    );
    let service_store = store.clone();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    ));
    let policy = crate::linux_policy::LinuxPolicy::fixture(&policy_path, clock.clone());
    let profile = crate::provider_supervisor::ProviderProfile::fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let socket = path.join("socket");
    let config = crate::config::Config {
        socket: socket.to_str().unwrap().into(),
        queued_per_authenticator: if separate_connection { 0 } else { 16 },
        ..Default::default()
    };
    std::thread::spawn(move || {
        let _ =
            crate::linux_service::serve(&config, service_store.as_ref(), &policy, &profile, clock);
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !socket.exists() {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    let transport =
        carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
            .unwrap();
    let system_client = separate_connection.then(|| {
        carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap()).unwrap()
    });
    let cancellation_client = system_client.as_ref().unwrap_or(&transport);
    let cancel = |key| {
        let mut acknowledged = false;
        let outcome = carosign_linux_ipc::client::mutation_events::observe_mutation_with_acceptance(
            &transport,
            Payload::BeginEnrollment(wire::BeginEnrollment {
                idempotency_key: vec![key; 16],
                subject: Some(common::SubjectSelector {
                    selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
                }),
                authenticator_id: vec![5; 16],
                label: "test".into(),
                replace: None,
                authority_request_ref: vec![],
            }),
            |operation| {
                acknowledged = true;
                if !send_cancel {
                    return;
                }
                if let Some(system) = &system_client {
                    let blocked = system
                        .request(Payload::BeginEnrollment(wire::BeginEnrollment {
                            idempotency_key: vec![key + 50; 16],
                            subject: Some(common::SubjectSelector {
                                selector: Some(common::subject_selector::Selector::CurrentSubject(
                                    true,
                                )),
                            }),
                            authenticator_id: vec![5; 16],
                            label: "queued".into(),
                            replace: None,
                            authority_request_ref: vec![],
                        }))
                        .unwrap();
                    assert!(
                        matches!(blocked.payload, Some(Payload::ProtocolError(error))
                        if error.error.as_ref().is_some_and(|error| error.code == common::ErrorCode::ErrorBusy as i32))
                    );
                }

                let receipt = cancellation_client
                    .request(Payload::CancelOperation(wire::CancelOperation {
                        operation_id: operation.to_vec(),
                    }))
                    .unwrap();
                assert!(matches!(
                    receipt.payload,
                    Some(Payload::CancellationReceipt(_))
                ));
            },
        );
        assert!(acknowledged, "key={key}, outcome={outcome:?}");
        outcome
    };
    let outcome = cancel(41);
    if expect_rollback {
        for result in [outcome, cancel(42)] {
            assert_eq!(
                result.unwrap().result,
                wire::MutationResult::MutationNoChange as i32
            );
        }
        let read = store.begin_read().unwrap();
        let scan = carosign_persistence::mutation_store::scan_prepared_v4_snapshot(&*read).unwrap();
        assert!(scan.blocked.is_empty());
        assert!(
            read.scan_prefix(Table::Enrollments, &[])
                .unwrap()
                .is_empty()
        );
        return;
    }
    assert!(
        outcome.is_err(),
        "отмена не должна выдать успешный терминал"
    );
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let read = store.begin_read().unwrap();
        let scan = carosign_persistence::mutation_store::scan_prepared_v4_snapshot(&*read).unwrap();
        if !scan.scan.needing_reconciliation.is_empty() {
            assert_eq!(scan.blocked.len(), 1);
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "область изменения не осталась закрытой"
        );
        drop(read);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // While the artist is still finishing, the picture may not be available, and once it is
    // completed, the health must show the retained area.
    let diagnostic =
        carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
            .unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let response = diagnostic
            .request(Payload::GetHealth(wire::GetHealth {}))
            .unwrap();
        if let Some(Payload::Health(health)) = response.payload {
            assert_eq!(health.scopes_needing_reconciliation, 1);
            assert!(!health.service_ready);
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "нет согласованного снимка состояния"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn mutation_interruption_keeps_scope(expire: bool) {
    let path = std::env::temp_dir().join(format!(
        "carosign-mutation-interrupt-{}-{expire}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    let policy_path = path.join("policy");
    let uid = unsafe { libc::getuid() };
    std::fs::write(
        &policy_path,
        format!("uid={uid} subject={uid} actions=enroll lifetime=1\n"),
    )
    .unwrap();
    let store = Arc::new(
        carosign_persistence::redb::backend::RedbStore::create_current(&path.join("store.redb"))
            .unwrap(),
    );
    let service_store = store.clone();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    ));
    let service_clock = clock.clone();
    let policy = crate::linux_policy::LinuxPolicy::fixture(&policy_path, clock.clone());
    let profile = crate::provider_supervisor::ProviderProfile::fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let socket = path.join("socket");
    let config = crate::config::Config {
        socket: socket.to_str().unwrap().into(),
        ..Default::default()
    };
    std::thread::spawn(move || {
        let _ = crate::linux_service::serve(
            &config,
            service_store.as_ref(),
            &policy,
            &profile,
            service_clock,
        );
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !socket.exists() {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    let transport =
        carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
            .unwrap();
    let response = transport
        .request(Payload::BeginEnrollment(wire::BeginEnrollment {
            idempotency_key: vec![42; 16],
            subject: Some(common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
            }),
            authenticator_id: vec![5; 16],
            label: "test".into(),
            replace: None,
            authority_request_ref: vec![],
        }))
        .unwrap();
    assert!(matches!(
        response.payload,
        Some(Payload::OperationAccepted(_))
    ));
    if expire {
        clock.advance(2_000_000_000);
    } else {
        drop(transport);
    }
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let read = store.begin_read().unwrap();
        let scan = carosign_persistence::mutation_store::scan_prepared_v4_snapshot(&*read).unwrap();
        if !scan.scan.needing_reconciliation.is_empty() {
            assert_eq!(scan.blocked.len(), 1);
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "неопределённая область не ограждена"
        );
        drop(read);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
#[ignore = "требует broker и mock в сценарии linux-pending-mutation"]
fn mutation_disconnect_keeps_scope_for_reconciliation() {
    mutation_interruption_keeps_scope(false);
}

#[test]
#[ignore = "требует broker и mock в сценарии linux-pending-mutation"]
fn mutation_deadline_keeps_scope_for_reconciliation() {
    mutation_interruption_keeps_scope(true);
}

#[test]
#[ignore = "требует broker и отдельный mock в сценарии seeded-pending-cancel"]
fn live_dispatcher_accepts_cancel_without_positive_result() {
    let path = std::env::temp_dir().join(format!("carosign-live-cancel-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let policy_dir = path.join("policies");
    std::fs::create_dir(&policy_dir).unwrap();
    let policy_path = policy_dir.join("policy");
    let uid = unsafe { libc::getuid() };
    std::fs::write(
        &policy_path,
        format!("uid={uid} subject={uid} actions=verify purpose=session-login\n"),
    )
    .unwrap();
    let store =
        carosign_persistence::redb::backend::RedbStore::create_current(&path.join("store.redb"))
            .unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes(*b"linux-local-v001").unwrap(),
        subject: carosign_adapters::policy::subject_of(u64::from(uid)),
    };
    let authenticator = carosign_core::ids::AuthenticatorId::from_bytes([5; 16]).unwrap();
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([15; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let record = EnrollmentRecord {
        subject,
        enrollment,
        display_label: None,
        label: "тест".into(),
        status: EnrollmentStatus::Active,
        provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(vec![21; 16]).unwrap(),
        format_id: String::new(),
        format_version: 0,
        binding_kind: carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque,
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        enrollment.id.as_bytes(),
        &carosign_persistence::mutation_store::encode_enrollment_v4(&record).unwrap(),
    )
    .unwrap();
    let mut key = crate::accept::scope_of(subject, authenticator).to_vec();
    key.extend_from_slice(enrollment.id.as_bytes());
    tx.put(Table::EnrollmentIndex, &key, enrollment.id.as_bytes())
        .unwrap();
    tx.commit_durable().unwrap();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    ));
    let policy = crate::linux_policy::LinuxPolicy::fixture(&policy_path, clock.clone());
    let profile = crate::provider_supervisor::ProviderProfile::fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let socket = path.join("socket");
    let config = crate::config::Config {
        socket: socket.to_str().unwrap().into(),
        ..Default::default()
    };
    std::thread::spawn(move || {
        let _ = crate::linux_service::serve(&config, &store, &policy, &profile, clock);
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !socket.exists() {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    if std::env::var_os("CAROSIGN_TEST_POLICY_REVOKE").is_some() {
        let transport =
            carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
                .unwrap();
        let request = wire::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: vec![43; 16],
            subject: Some(common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
            }),
            authenticator_id: authenticator.as_bytes().to_vec(),
            purpose_hint: common::VerificationPurpose::PurposeSessionLogin as i32,
            authority_request_ref: vec![],
        };
        let start = std::time::Instant::now();
        let result =
            pam_carosign::client::observe_verification_with_acceptance(&transport, request, |_| {
                std::thread::sleep(std::time::Duration::from_millis(300));
                std::fs::write(&policy_path, "# отозвано\n").unwrap();
            });
        assert_ne!(
            result,
            Ok(pam_carosign::client::Observation::FactorAccepted)
        );
        assert!(
            start.elapsed() < std::time::Duration::from_secs(4),
            "отзыв не остановил ожидающего работника"
        );
        return;
    }
    if std::env::var_os("CAROSIGN_TEST_LONG_WAIT").is_some() {
        let transport = Arc::new(
            carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
                .unwrap(),
        );
        let request = wire::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: vec![44; 16],
            subject: Some(common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
            }),
            authenticator_id: authenticator.as_bytes().to_vec(),
            purpose_hint: common::VerificationPurpose::PurposeSessionLogin as i32,
            authority_request_ref: vec![],
        };
        let cancellation = transport.clone();
        let started = std::time::Instant::now();
        let result = pam_carosign::client::observe_verification_with_acceptance(
            &transport,
            request,
            move |operation_id| {
                let operation_id = operation_id.to_vec();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(6));
                    cancellation
                        .request(Payload::CancelOperation(wire::CancelOperation {
                            operation_id,
                        }))
                        .unwrap();
                });
            },
        );
        assert_eq!(result, Ok(pam_carosign::client::Observation::Cancelled));
        assert!(started.elapsed() >= std::time::Duration::from_secs(6));
        return;
    }
    // A customer gap during operation should not leave the device occupied.
    let disconnected =
        carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
            .unwrap();
    let answer = disconnected
        .request(Payload::BeginVerification(wire::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: vec![42; 16],
            subject: Some(common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
            }),
            authenticator_id: authenticator.as_bytes().to_vec(),
            purpose_hint: common::VerificationPurpose::PurposeSessionLogin as i32,
            authority_request_ref: vec![],
        }))
        .unwrap();
    assert!(matches!(
        answer.payload,
        Some(Payload::OperationAccepted(_))
    ));
    // Allow time to start the worker: the gap checks the operation already underway.
    std::thread::sleep(std::time::Duration::from_millis(300));
    drop(disconnected);
    std::thread::sleep(std::time::Duration::from_millis(500));
    for attempt in 1..=2 {
        let transport =
            carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
                .unwrap();
        let request = wire::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: vec![attempt; 16],
            subject: Some(common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
            }),
            authenticator_id: authenticator.as_bytes().to_vec(),
            purpose_hint: common::VerificationPurpose::PurposeSessionLogin as i32,
            authority_request_ref: vec![],
        };
        let mut receipt = false;
        let result = pam_carosign::client::observe_verification_with_acceptance(
            &transport,
            request,
            |operation_id| {
                if let Ok(answer) =
                    transport.request(Payload::CancelOperation(wire::CancelOperation {
                        operation_id: operation_id.to_vec(),
                    }))
                {
                    assert!(
                        matches!(answer.payload, Some(Payload::CancellationReceipt(ref r)) if r.result == wire::CancelReceiptKind::CancelReceiptRequested as i32)
                    );
                    receipt = true;
                }
            },
        );
        assert!(
            receipt,
            "попытка {attempt}: диспетчер не подтвердил принятую отмену"
        );
        assert_eq!(result, Ok(pam_carosign::client::Observation::Cancelled));
    }
}
