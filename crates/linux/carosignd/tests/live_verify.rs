//! Live conversation with verification: socket, client, greeting, request, result.
//!
//! Exercises kernel-supplied `SO_PEERCRED`, `accept4`, and delivery over a real socket.

#[path = "live_verify_fixtures/allow.rs"]
mod allow;
#[path = "live_verify_fixtures/child_guard.rs"]
mod child_guard;
#[path = "live_verify_fixtures/clock.rs"]
mod clock;
#[path = "live_verify_fixtures/fixture_health.rs"]
mod fixture_health;
#[path = "live_verify_fixtures/ids.rs"]
mod ids;
#[path = "live_verify_fixtures/journal.rs"]
mod journal;
#[path = "live_verify_fixtures/provider.rs"]
mod provider;
#[path = "live_verify_fixtures/unavailable_authority.rs"]
mod unavailable_authority;

use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

use carosign_core::authorization::{AuthorizationBoundary, GrantScope, ValidatedGrant};
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::IdSource;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, BootId, CommandId, ConnectionId, EnrollmentId, EvidenceId,
    GrantId, IdempotencyKey, LeaseId, MutationId, OperationId, SubjectId, WorkerInstanceId,
};
use carosign_core::operation::context::BootInstant;
use carosign_linux::sys;
use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
use carosign_persistence::mutation_plan::StoredBindingKind;
use carosign_persistence::redb::backend::RedbStore;
use carosign_persistence::store::{MetadataStore, Table};
use carosign_protocol::carosign::client::v1::client_frame::Payload;
use carosign_protocol::carosign::client::v1::{self as client, ClientFrame};
use carosign_protocol::carosign::common::v1 as common;
use carosign_runtime::ports::{
    AuthorityRefusal, BoxFuture, CallerContext, CommandPlan, DispatchOutcome, DurableMarker,
    MutationPort, PolicyAuthority, ProviderExecutionPort, TimeSource,
};
mod support;
use carosignd::server::{accept_one, new_connection};
use carosignd::service::Service;
use carosignd::talk::{Asked, Ended, converse};
use prost::Message;
use support::GrantAll;

fn temp_socket(name: &str) -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("talk-{name}-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path.to_str().expect("путь").to_string()
}

fn temp_store(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("talk-{name}-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn authenticator() -> AuthenticatorId {
    AuthenticatorId::from_bytes([3u8; 16]).expect("валид")
}

fn record() -> EnrollmentRecord {
    EnrollmentRecord {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([5u8; 16]).expect("валид"),
            authenticator: authenticator(),
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "палец".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![1, 2, 3]).expect("годится"),
        format_id: "goodix-asp".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    }
}

fn put(store: &RedbStore, record: &EnrollmentRecord) {
    let mut tx = store.begin_write().expect("запись");
    let bytes = carosign_persistence::enrollment_codec::encode_v3(record).expect("кодируется");
    tx.put(Table::Enrollments, record.enrollment.id.as_bytes(), &bytes)
        .expect("положено");
    // ⚠️ The index key is the registry area: `authority ‖ subject ‖ authenticator`It's 48
    // bytes, and without it, the registry will say, "No registrations," even though it does.
    let scope = carosignd::accept::scope_of(record.subject, record.enrollment.authenticator);
    let mut index = [0u8; carosign_persistence::limits::ENROLLMENT_INDEX_KEY_LEN];
    index[..scope.len()].copy_from_slice(&scope);
    index[scope.len()..].copy_from_slice(record.enrollment.id.as_bytes());
    tx.put(
        Table::EnrollmentIndex,
        &index,
        record.enrollment.id.as_bytes(),
    )
    .expect("индекс");
    tx.commit_durable().expect("зафиксировано");
}

use ids::Ids;

use clock::Clock;

use allow::Allow;

use provider::Provider;

use journal::Journal;

fn hello() -> Vec<u8> {
    client::ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: vec![1u8; 16],
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::Hello(client::Hello {
            capture_feedback_v1: false,
            major: 1,
            min_minor: carosign_protocol::limits::MINOR_BASELINE,
            max_minor: carosign_protocol::limits::MINOR_MAX,
        })),
    }
    .encode_to_vec()
}

fn begin_verification(authenticator: &AuthenticatorId) -> Vec<u8> {
    client::ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: vec![2u8; 16],
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::BeginVerification(client::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: vec![3u8; 16],
            subject: Some(common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
            }),
            authenticator_id: authenticator.as_bytes().to_vec(),
            purpose_hint: common::VerificationPurpose::PurposeSessionLogin as i32,
            authority_request_ref: Vec::new(),
        })),
    }
    .encode_to_vec()
}

fn send(stream: &UnixStream, bytes: &[u8]) {
    // SAFETY: The descriptor is valid, the buffer is ours.
    let rc = unsafe {
        libc::send(
            stream.as_raw_fd(),
            bytes.as_ptr() as *const core::ffi::c_void,
            bytes.len(),
            libc::MSG_NOSIGNAL,
        )
    };
    assert_eq!(rc as usize, bytes.len(), "кадр не отправлен");
}

fn receive(stream: &UnixStream) -> Vec<u8> {
    let mut buffer = vec![0u8; carosign_protocol::limits::FRAME_MAX];
    // SAFETY: The descriptor is valid, and the kernel receives the owned buffer and its length.
    let rc = unsafe {
        libc::recv(
            stream.as_raw_fd(),
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
            buffer.len(),
            0,
        )
    };
    assert!(
        rc > 0,
        "кадр не прочитан: {}",
        std::io::Error::last_os_error()
    );
    buffer.truncate(rc as usize);
    buffer
}

fn seqpacket_connect(path: &str) -> UnixStream {
    use std::os::fd::FromRawFd;
    // SAFETY: regular calls; the result is checked.
    unsafe {
        let fd = libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0);
        assert!(fd >= 0, "сокет не открылся");
        let mut address: libc::sockaddr_un = core::mem::zeroed();
        address.sun_family = libc::AF_UNIX as libc::sa_family_t;
        let bytes = path.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            address.sun_path[index] = *byte as libc::c_char;
        }
        let length =
            (core::mem::size_of::<libc::sa_family_t>() + bytes.len() + 1) as libc::socklen_t;
        assert_eq!(
            libc::connect(
                fd,
                &address as *const libc::sockaddr_un as *const libc::sockaddr,
                length,
            ),
            0,
            "подключение не удалось"
        );
        UnixStream::from_raw_fd(fd)
    }
}

#[test]
fn live_verification_rejects_incomplete_context_before_provider() {
    let path = temp_socket("verify");
    let store_path = temp_store("verify");
    let store = RedbStore::create(&store_path).expect("создано");
    put(&store, &record());
    let listener = sys::listen_unix(&path).expect("слушатель");

    let client_path = path.clone();
    let client = std::thread::spawn(move || {
        let stream = seqpacket_connect(&client_path);
        send(&stream, &hello());
        let ack = ClientFrame::decode(&receive(&stream)[..]).expect("ответ на приветствие");
        assert!(matches!(ack.payload, Some(Payload::HelloAck(_))));

        let mut invalid = ClientFrame::decode(&begin_verification(&authenticator())[..]).unwrap();
        invalid.request_id = vec![42; 16];
        if let Some(Payload::BeginVerification(request)) = &mut invalid.payload {
            request.subject = None;
        }
        send(&stream, &invalid.encode_to_vec());
        let refused = ClientFrame::decode(&receive(&stream)[..]).unwrap();
        assert_eq!(refused.request_id, vec![42; 16]);
        assert!(
            matches!(refused.payload, Some(Payload::ProtocolError(error)) if error.error.as_ref().unwrap().code == common::ErrorCode::ErrorInvalidMessage as i32)
        );

        for (index, selector) in [
            common::subject_selector::Selector::CurrentSubject(true),
            common::subject_selector::Selector::AccountHint("другой пользователь".into()),
            common::subject_selector::Selector::OpaqueAuthoritySelector(vec![4, 5, 6]),
        ]
        .into_iter()
        .enumerate()
        {
            let mut frame = ClientFrame::decode(&begin_verification(&authenticator())[..]).unwrap();
            frame.request_id = vec![50 + index as u8; 16];
            if let Some(Payload::BeginVerification(request)) = &mut frame.payload {
                request.subject = Some(common::SubjectSelector {
                    selector: Some(selector),
                });
                request.purpose_hint = (index + 1) as i32;
                request.authority_request_ref = vec![9, 8, 7, index as u8];
            }
            send(&stream, &frame.encode_to_vec());
            let response = ClientFrame::decode(&receive(&stream)[..]).expect("ответ на проверку");
            assert_eq!(response.request_id, frame.request_id);
            assert!(
                matches!(response.payload, Some(Payload::ProtocolError(error)) if error.error.as_ref().unwrap().code == common::ErrorCode::ErrorUnavailable as i32)
            );
        }
        // The two oneof assignments are a damaged frame, not a single request rejection. The
        // service closes the unanswered connection to the blank ID.
        let mut ambiguous = begin_verification(&authenticator());
        ambiguous.extend_from_slice(&[0xf2, 0x01, 0x00]); // GetHealth
        send(&stream, &ambiguous);
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut byte = [0u8; 1];
        // SAFETY: The buffer and descriptor remain valid until recv returns.
        assert_eq!(
            unsafe { libc::recv(stream.as_raw_fd(), byte.as_mut_ptr().cast(), 1, 0) },
            0
        );

        let stream = seqpacket_connect(&client_path);
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut incompatible = ClientFrame::decode(&hello()[..]).unwrap();
        incompatible.request_id = vec![72; 16];
        incompatible.major = 2;
        if let Some(Payload::Hello(hello)) = &mut incompatible.payload {
            hello.major = 2;
        }
        send(&stream, &incompatible.encode_to_vec());
        let response = ClientFrame::decode(&receive(&stream)[..]).unwrap();
        assert_eq!(response.request_id, incompatible.request_id);
        assert!(
            matches!(response.payload, Some(Payload::ProtocolError(error)) if error.error.as_ref().unwrap().code == common::ErrorCode::ErrorUnsupportedVersion as i32)
        );
        // SAFETY: The buffer and descriptor remain valid until recv returns.
        assert_eq!(
            unsafe { libc::recv(stream.as_raw_fd(), byte.as_mut_ptr().cast(), 1, 0) },
            0
        );

        // The next connection remains functional and receives a response with the same ID
        // when the incompatible version is closed.
        let stream = seqpacket_connect(&client_path);
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        send(&stream, &hello());
        assert!(matches!(
            ClientFrame::decode(&receive(&stream)[..]).unwrap().payload,
            Some(Payload::HelloAck(_))
        ));
        let mut version = ClientFrame::decode(&begin_verification(&authenticator())[..]).unwrap();
        version.request_id = vec![71; 16];
        version.major = 2;
        send(&stream, &version.encode_to_vec());
        let response = ClientFrame::decode(&receive(&stream)[..]).unwrap();
        assert_eq!(response.request_id, version.request_id);
        assert!(
            matches!(response.payload, Some(Payload::ProtocolError(error)) if error.error.as_ref().unwrap().code == common::ErrorCode::ErrorUnsupportedVersion as i32)
        );
        // SAFETY: The buffer and descriptor remain valid until recv returns.
        assert_eq!(
            unsafe { libc::recv(stream.as_raw_fd(), byte.as_mut_ptr().cast(), 1, 0) },
            0
        );
    });

    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal;
    let resources = GrantAll {
        epoch: DeviceEpoch::FIRST,
    };
    let service = Service {
        store: &store,
        authority: &Allow,
        ids: &ids,
        clock: &Clock,
        resources: &resources,
        resource_sets: &support::TestResourceSets,
        operations: &carosignd::operations::OperationRegistry::new(),
        provider: &provider,
        mutation: &journal,
    };
    let mut connection = accept_one(&listener, new_connection().unwrap()).expect("принято");
    let ended = futures_executor::block_on(converse(
        &mut connection,
        &service,
        Asked {
            subject: subject(),
            epoch: DeviceEpoch::FIRST,
        },
    ));
    assert!(matches!(ended, Ended::Refused(_)));
    drop(connection);
    let mut connection =
        accept_one(&listener, new_connection().unwrap()).expect("второе соединение");
    let ended = futures_executor::block_on(converse(
        &mut connection,
        &service,
        Asked {
            subject: subject(),
            epoch: DeviceEpoch::FIRST,
        },
    ));
    assert!(matches!(ended, Ended::Refused(_)));
    drop(connection);

    let mut connection =
        accept_one(&listener, new_connection().unwrap()).expect("третье соединение");
    let ended = futures_executor::block_on(converse(
        &mut connection,
        &service,
        Asked {
            subject: subject(),
            epoch: DeviceEpoch::FIRST,
        },
    ));
    assert!(matches!(ended, Ended::Refused(_)));
    drop(connection);

    client.join().expect("клиент");
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&store_path);
}

#[test]
fn live_request_without_enrollment_is_refused_before_registry_access() {
    let path = temp_socket("none");
    let store_path = temp_store("none");
    let store = RedbStore::create(&store_path).expect("создано");
    let listener = sys::listen_unix(&path).expect("слушатель");

    let client_path = path.clone();
    let client = std::thread::spawn(move || {
        let stream = seqpacket_connect(&client_path);
        send(&stream, &hello());
        let _ = receive(&stream);
        send(&stream, &begin_verification(&authenticator()));
        ClientFrame::decode(&receive(&stream)[..]).expect("отказ")
    });

    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal;
    let resources = GrantAll {
        epoch: DeviceEpoch::FIRST,
    };
    let service = Service {
        store: &store,
        authority: &Allow,
        ids: &ids,
        clock: &Clock,
        resources: &resources,
        resource_sets: &support::TestResourceSets,
        operations: &carosignd::operations::OperationRegistry::new(),
        provider: &provider,
        mutation: &journal,
    };
    let mut connection = accept_one(&listener, new_connection().unwrap()).expect("принято");
    let ended = futures_executor::block_on(converse(
        &mut connection,
        &service,
        Asked {
            subject: subject(),
            epoch: DeviceEpoch::FIRST,
        },
    ));
    assert_eq!(ended, Ended::Closed);

    match client.join().expect("клиент").payload {
        Some(Payload::ProtocolError(error)) => {
            let error = error.error.expect("ошибка не пуста");
            assert_eq!(error.code, common::ErrorCode::ErrorUnavailable as i32);
        }
        other => panic!("ожидался отказ, а пришло {other:?}"),
    }
    // ⚠️ Provider not affected: there was no operation.
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&store_path);
}

/// A separate test process: it's not a production bootstrap, it's not an indication of
/// service readiness, and the authority and the broker are intentionally unavailable.
#[test]
#[ignore]
fn fixture_process() {
    use unavailable_authority::UnavailableAuthority;

    let path = std::env::var("CAROSIGN_FIXTURE_SOCKET").expect("путь стенда");
    let store_path = std::env::var("CAROSIGN_FIXTURE_STORE").expect("путь хранилища стенда");
    let store = RedbStore::create_v3(std::path::Path::new(&store_path)).unwrap();
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let registry = carosignd::operations::OperationRegistry::new();
    let service = Service {
        store: &store,
        authority: &UnavailableAuthority,
        ids: &ids,
        clock: &Clock,
        resources: &carosignd::resources::NoBroker,
        resource_sets: &support::TestResourceSets,
        operations: &registry,
        provider: &provider,
        mutation: &Journal,
    };
    let listener = sys::listen_unix(&path).unwrap();
    let count = if std::env::var_os("CAROSIGN_FIXTURE_CLI").is_some() {
        2
    } else {
        1
    };
    for _ in 0..count {
        let mut connection = accept_one(&listener, new_connection().unwrap()).unwrap();
        use fixture_health::FixtureHealth;
        let asked = Asked {
            subject: subject(),
            epoch: DeviceEpoch::FIRST,
        };
        let ended = if std::env::var_os("CAROSIGN_FIXTURE_HEALTH").is_some() {
            futures_executor::block_on(carosignd::talk::converse_with_health(
                &mut connection,
                &service,
                asked,
                &FixtureHealth(std::sync::atomic::AtomicU32::new(0)),
            ))
        } else {
            futures_executor::block_on(converse(&mut connection, &service, asked))
        };
        assert_eq!(ended, Ended::Closed);
    }
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn separate_fixture_process_exchanges_real_frames_without_authorizing() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use child_guard::ChildGuard;

    let path = temp_socket("process-fixture");
    let store_path = temp_store("process-fixture");
    let cli = std::env::var_os("CAROSIGNCTL_TEST_BINARY");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "fixture_process", "--ignored", "--nocapture"])
        .env("CAROSIGN_FIXTURE_SOCKET", &path)
        .env("CAROSIGN_FIXTURE_STORE", &store_path)
        .stdout(Stdio::null());
    if cli.is_some() {
        command.env("CAROSIGN_FIXTURE_CLI", "1");
    }
    let mut child = ChildGuard(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !std::path::Path::new(&path).exists() {
        assert!(child.0.try_wait().unwrap().is_none(), "стенд завершился");
        assert!(Instant::now() < deadline, "сокет стенда не появился");
        std::thread::sleep(Duration::from_millis(10));
    }
    if let Some(cli) = cli {
        let output = Command::new(cli)
            .args(["--socket", &path, "health"])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "недоступная диагностика не является успехом"
        );
        assert!(!output.stderr.is_empty());
    }
    let stream = seqpacket_connect(&path);
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    send(&stream, &hello());
    let ack = ClientFrame::decode(&receive(&stream)[..]).unwrap();
    assert!(matches!(ack.payload, Some(Payload::HelloAck(_))));

    let health = ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: vec![11; 16],
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::GetHealth(client::GetHealth {})),
    };
    send(&stream, &health.encode_to_vec());
    let response = ClientFrame::decode(&receive(&stream)[..]).unwrap();
    assert_eq!(response.request_id, health.request_id);
    assert!(
        matches!(response.payload, Some(Payload::ProtocolError(error))
        if error.error.as_ref().unwrap().code == common::ErrorCode::ErrorUnavailable as i32)
    );

    let list = ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: vec![12; 16],
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::ListAuthenticators(client::ListAuthenticators {})),
    };
    send(&stream, &list.encode_to_vec());
    let response = ClientFrame::decode(&receive(&stream)[..]).unwrap();
    assert_eq!(response.request_id, list.request_id);
    assert!(
        matches!(response.payload, Some(Payload::ProtocolError(error))
        if error.error.as_ref().unwrap().code == common::ErrorCode::ErrorUnavailable as i32)
    );

    send(&stream, &begin_verification(&authenticator()));
    let response = ClientFrame::decode(&receive(&stream)[..]).unwrap();
    assert_eq!(response.request_id, vec![2; 16]);
    assert!(
        matches!(response.payload, Some(Payload::ProtocolError(error))
        if error.error.as_ref().unwrap().code == common::ErrorCode::ErrorUnavailable as i32)
    );
    drop(stream);
    assert!(child.0.wait().unwrap().success());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(store_path).unwrap();
}

#[test]
fn separate_fixture_process_serves_health_snapshot() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use child_guard::ChildGuard;

    let path = temp_socket("health-fixture");
    let store_path = temp_store("health-fixture");
    let cli = std::env::var_os("CAROSIGNCTL_TEST_BINARY");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "fixture_process", "--ignored", "--nocapture"])
        .env("CAROSIGN_FIXTURE_SOCKET", &path)
        .env("CAROSIGN_FIXTURE_STORE", &store_path)
        .env("CAROSIGN_FIXTURE_HEALTH", "1")
        .stdout(Stdio::null());
    if cli.is_some() {
        command.env("CAROSIGN_FIXTURE_CLI", "1");
    }
    let mut child = ChildGuard(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !std::path::Path::new(&path).exists() {
        assert!(child.0.try_wait().unwrap().is_none(), "стенд завершился");
        assert!(Instant::now() < deadline, "сокет стенда не появился");
        std::thread::sleep(Duration::from_millis(10));
    }
    if let Some(cli) = cli {
        let output = Command::new(cli)
            .args(["--socket", &path, "health"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        let output = String::from_utf8(output.stdout).unwrap();
        assert!(
            output.contains("готова")
                && output.contains("доступны: 2")
                && output.contains("недоступны: 3")
                && output.contains("сверки: 0")
        );
    }
    let stream = seqpacket_connect(&path);
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    send(&stream, &hello());
    let ack = ClientFrame::decode(&receive(&stream)[..]).unwrap();
    assert!(matches!(ack.payload, Some(Payload::HelloAck(_))));
    let request = ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: vec![11; 16],
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::GetHealth(client::GetHealth {})),
    };
    send(&stream, &request.encode_to_vec());
    let response = ClientFrame::decode(&receive(&stream)[..]).unwrap();
    assert_eq!(response.request_id, request.request_id);
    assert!(
        matches!(response.payload, Some(Payload::Health(h)) if h.service_ready && h.available_authenticators == 2 && h.unavailable_authenticators == 3 && h.scopes_needing_reconciliation == 0)
    );
    send(&stream, &request.encode_to_vec());
    let updated = ClientFrame::decode(&receive(&stream)[..]).unwrap();
    assert!(matches!(updated.payload, Some(Payload::Health(h)) if h.available_authenticators == 3));
    let list = ClientFrame {
        request_id: vec![12; 16],
        payload: Some(Payload::ListAuthenticators(client::ListAuthenticators {})),
        ..request
    };
    send(&stream, &list.encode_to_vec());
    let response = ClientFrame::decode(&receive(&stream)[..]).unwrap();
    assert!(
        matches!(response.payload, Some(Payload::ProtocolError(error)) if error.error.as_ref().unwrap().code == common::ErrorCode::ErrorUnavailable as i32)
    );
    drop(stream);
    assert!(child.0.wait().unwrap().success());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(store_path).unwrap();
}
