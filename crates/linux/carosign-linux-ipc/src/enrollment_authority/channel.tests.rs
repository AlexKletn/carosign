use super::super::message::{Approved, Empty, OwnerBinding, PlanBinding, PlanChange, Validate};
use super::*;
use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, BootId, CommandId, LeaseId, MutationId, OperationId, SubjectId,
    WorkerInstanceId,
};
use carosign_runtime::enrollment_authorization::EnrollmentRecipientId;
use sha2::{Digest, Sha256};
use std::os::fd::FromRawFd;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

fn scope() -> EnrollmentAuthorizationScope {
    EnrollmentAuthorizationScope {
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
        recipient: EnrollmentRecipientId::from_bytes([8; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([9; 16]).unwrap(),
        lease: LeaseId::from_bytes([10; 16]).unwrap(),
        boot: BootId::from_bytes([11; 16]).unwrap(),
    }
}

fn pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [-1; 2];
    // SAFETY: fds contains two correct places for the result.
    assert_eq!(
        unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        },
        0
    );
    // SAFETY: A successful socketpair transfers unique ownership.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn peer() -> ExpectedPeer {
    // SAFETY: Getpid/geteuid does not require preconditions.
    unsafe {
        ExpectedPeer {
            pid: libc::getpid() as u32,
            uid: libc::geteuid(),
        }
    }
}

fn grant() -> GrantBinding {
    GrantBinding {
        id: vec![12; 16],
        issuer: vec![13; 16],
        deadline_ns: 10_000,
    }
}

#[test]
fn send_boundary_excludes_revocation_only_until_packet_is_sent() {
    use std::sync::{Arc, Mutex, mpsc};
    let scope = scope();
    let (client, server) = pair();
    let mut client = AuthorityChannel::new(client, peer(), &scope).unwrap();
    let mut server = AuthorityChannel::new(server, peer(), &scope).unwrap();
    let lease = Arc::new(Mutex::new(true));
    let (arrived_tx, arrived_rx) = mpsc::channel();
    let (reply_tx, reply_rx) = mpsc::channel();
    let server_thread = std::thread::spawn(move || {
        server
            .serve_one(Duration::from_secs(3), |_, action| {
                assert!(matches!(action, request::Action::Execute(_)));
                arrived_tx.send(()).unwrap();
                reply_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                response::Result::Accepted(Empty {})
            })
            .unwrap();
    });
    let sender_lease = lease.clone();
    let sender = std::thread::spawn(move || {
        let mut boundary = |send: &mut dyn FnMut() -> bool| {
            let live = sender_lease.lock().unwrap();
            *live && send()
        };
        client.request_with_boundary(
            grant(),
            request::Action::Execute(Authorization {
                reference: vec![15; 16],
                challenge_digest: vec![16; 32],
            }),
            Duration::from_secs(3),
            &mut boundary,
        )
    });
    arrived_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    // The receiver withholds the response, but the recall does not wait for the network
    // operation.
    *lease.lock().unwrap() = false;
    reply_tx.send(()).unwrap();
    assert!(matches!(
        sender.join().unwrap(),
        Ok(response::Result::Accepted(_))
    ));
    server_thread.join().unwrap();

    let (client, server) = pair();
    let mut client = AuthorityChannel::new(client, peer(), &scope).unwrap();
    let server = AuthorityChannel::new(server, peer(), &scope).unwrap();
    let mut boundary = |send: &mut dyn FnMut() -> bool| {
        let live = lease.lock().unwrap();
        *live && send()
    };
    assert!(
        client
            .request_with_boundary(
                grant(),
                request::Action::Execute(Authorization {
                    reference: vec![15; 16],
                    challenge_digest: vec![16; 32]
                }),
                Duration::from_secs(1),
                &mut boundary
            )
            .is_err()
    );
    assert!(
        server
            .receive(Instant::now() + Duration::from_secs(1))
            .is_err()
    );
}

#[test]
fn plan_wire_requires_consistent_typed_change() {
    let mut plan = PlanBinding {
        planned_enrollment: vec![1; 16],
        generation: 1,
        plan_digest: vec![2; 32],
        change: PlanChange::Create as i32,
        old_enrollment: vec![],
        old_generation: 0,
    };
    assert!(valid_action(&request::Action::BindPlan(plan.clone())));
    plan.old_enrollment = vec![3; 16];
    plan.old_generation = 1;
    assert!(!valid_action(&request::Action::BindPlan(plan.clone())));
    plan.change = PlanChange::Replace as i32;
    assert!(valid_action(&request::Action::BindPlan(plan.clone())));
    plan.change = PlanChange::Create as i32;
    assert!(!valid_action(&request::Action::BindPlan(plan)));
}

#[test]
fn admitted_execute_reaches_owner_process_only_with_durable_ticket() {
    use carosign_core::authorization::{GrantScope, ValidatedGrant};
    use carosign_core::enrollment::EnrollmentLabel;
    use carosign_core::ids::{EnrollmentId, GrantId, IdempotencyKey};
    use carosign_core::operation::{context::BootInstant, plan::PlanTransition};
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    use carosign_persistence::journal::{ArmMutation, PrepareMutation};
    use carosign_persistence::model::IdempotencyRecord;
    use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};
    use carosign_persistence::redb::backend::RedbStore;
    use carosign_runtime::enrollment_admission::{
        ConsumptionOutcome, SerializedEnrollmentAdmission,
    };
    use carosign_runtime::enrollment_authorization::{
        AuthorizedEnrollment, ChallengeObservation, ValidatedChallenge,
    };
    use carosign_runtime::operation::registry::ManualClock;
    use carosign_runtime::ports::{
        CommandPlan, EnrollmentAuthorization, MutationArmRequest, MutationChange,
    };
    use std::os::unix::process::CommandExt;

    let scope = scope();
    if let Ok(value) = std::env::var("CAROSIGN_TEST_OWNER_FD") {
        let fd: i32 = value.parse().unwrap();
        // SAFETY: The parent transmitted the only open descriptor via exec.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let expected = ExpectedPeer {
            pid: std::env::var("CAROSIGN_TEST_PARENT_PID")
                .unwrap()
                .parse()
                .unwrap(),
            uid: unsafe { libc::geteuid() },
        };
        let (mut server, received_scope, received_grant, payload) =
            AuthorityChannel::bootstrap_server(fd, expected, Duration::from_secs(5)).unwrap();
        assert_eq!(received_scope, scope);
        assert_eq!(received_grant.id, vec![12; 16]);
        assert!(payload.is_empty());
        server
            .serve_one(Duration::from_secs(5), |_, action| {
                assert!(matches!(action, request::Action::PrepareChallenge(_)));
                response::Result::Challenge(Validate {
                    challenge: vec![1],
                    challenge_ref: vec![14; 16],
                    validity_ms: 500,
                })
            })
            .unwrap();
        server.serve_one(Duration::from_secs(5), |_, action| {
                assert!(matches!(action, request::Action::BindOwner(binding) if binding.payload == b"owner binding"));
                response::Result::Accepted(Empty {})
            }).unwrap();
        server
            .serve_one(Duration::from_secs(5), |_, action| {
                assert!(matches!(action, request::Action::Execute(_)));
                response::Result::Accepted(Empty {})
            })
            .unwrap();
        return;
    }
    let grant = ValidatedGrant::issue_for_port(
        GrantId::from_bytes([12; 16]).unwrap(),
        AuthorityId::from_bytes([13; 16]).unwrap(),
        GrantScope::Enroll {
            subject: scope.subject,
            authenticator: scope.authenticator.id,
            replace: None,
        },
        scope.boot,
        2_000_000_000,
    );
    let observation = ChallengeObservation::new(vec![1], [14; 16], 500, 500).unwrap();
    let challenge = ValidatedChallenge::issued_for_port(
        scope,
        &observation,
        &grant,
        BootInstant::new(scope.boot, 1_000_000_000),
        BootInstant::new(scope.boot, 1_500_000_000),
    )
    .unwrap();
    let authorization = AuthorizedEnrollment::issued_for_port(&challenge, [15; 16]).unwrap();
    let domain_scope = Scope {
        subject: scope.subject,
        authenticator: scope.authenticator.id,
    };
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([16; 16]).unwrap()),
        1,
        Vec::new(),
        domain_scope,
    )
    .unwrap()
    .with_requested_label(domain_scope, "finger")
    .unwrap();
    let core_plan = plan.to_core(domain_scope).unwrap();
    let PlanTransition::Create { planned_new } = core_plan.transition() else {
        panic!("create")
    };
    let input = PrepareMutation {
        mutation: scope.mutation,
        operation: scope.operation,
        subject: scope.subject,
        authenticator: scope.authenticator.id,
        expected_generation: None,
        enroll: true,
        plan,
    };
    let idem = IdempotencyRecord {
        caller_digest: [17; 32],
        key: IdempotencyKey::from_bytes([18; 16]).unwrap(),
        request_digest: carosign_persistence::mutation_plan::canonical_request_digest(
            domain_scope,
            Transition::EnrollNew,
            None,
            Some("finger"),
        )
        .unwrap(),
        operation: scope.operation,
        mutation: Some(scope.mutation),
    };
    let path = std::env::temp_dir().join(format!("carosign-ipc-admission-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let store = RedbStore::create_current(&path.join("store.redb")).unwrap();
    let fresh =
        carosign_persistence::mutation_store::prepare_fresh_arm_v4(&store, &input, &idem).unwrap();
    let arm = ArmMutation {
        mutation: scope.mutation,
        worker: scope.worker,
        epoch: scope.authenticator.device_epoch,
        command: scope.command,
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(1), 0);
    let ticket = fresh.arm(&arm, effect).unwrap();
    let mut scope_key = [0; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN];
    scope_key[..32].copy_from_slice(&scope.subject.to_canonical());
    scope_key[32..].copy_from_slice(scope.authenticator.id.as_bytes());
    let request = MutationArmRequest {
        operation: scope.operation,
        uid: scope.uid,
        effect,
        arm,
        scope: scope_key,
        lease: scope.lease,
        label: Some(EnrollmentLabel::new("finger").unwrap()),
        plan: CommandPlan::Mutate {
            mutation: scope.mutation,
            command: scope.command,
            authorization: EnrollmentAuthorization::Reference {
                reference: *authorization.reference(),
                challenge_digest: *authorization.digest(),
            },
            change: MutationChange::Create { planned_new },
        },
        logical_plan: core_plan,
    };
    let clock = ManualClock::new(scope.boot);
    clock.advance(1_000_000_000);
    let gate = SerializedEnrollmentAdmission::new(scope, &authorization, || true).unwrap();
    let (client, server) = pair();
    let fd = server.as_raw_fd();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    let (_, test_module) = module_path!().split_once("::").unwrap();
    command
        .arg("--exact")
        .arg(format!(
            "{test_module}::admitted_execute_reaches_owner_process_only_with_durable_ticket"
        ))
        .arg("--nocapture")
        .env("CAROSIGN_TEST_OWNER_FD", fd.to_string())
        .env("CAROSIGN_TEST_PARENT_PID", std::process::id().to_string());
    // SAFETY: In the child process before exec, only the flag of the transmitted fd is
    // changed.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = command.spawn().unwrap();
    drop(server);
    let mut client = AuthorityChannel::bootstrap_client(
        client,
        peer(),
        &scope,
        GrantBinding::from_grant(&grant),
        Vec::new(),
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(
        matches!(client.request(GrantBinding::from_grant(&grant), request::Action::PrepareChallenge(Empty {}), Duration::from_secs(5)), Ok(response::Result::Challenge(value)) if value.challenge == vec![1] && value.challenge_ref == vec![14; 16])
    );
    assert!(matches!(
        client.request(
            GrantBinding::from_grant(&grant),
            request::Action::BindOwner(OwnerBinding {
                payload: b"owner binding".to_vec()
            }),
            Duration::from_secs(5)
        ),
        Ok(response::Result::Accepted(_))
    ));
    let result = client
        .execute_admitted(
            &gate,
            &scope,
            &authorization,
            &grant,
            &clock,
            &store,
            &request,
            ticket,
            Duration::from_secs(5),
            || true,
            |_| ConsumptionOutcome::Completed,
        )
        .unwrap();
    assert_eq!(result, ConsumptionOutcome::Completed);
    assert!(child.wait().unwrap().success());
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn execute_acknowledges_completion_and_lost_reply_is_unknown() {
    use carosign_runtime::enrollment_admission::ConsumptionOutcome;
    let scope = scope();
    let (client, server) = pair();
    let mut client = AuthorityChannel::new(client, peer(), &scope).unwrap();
    let mut server = AuthorityChannel::new(server, peer(), &scope).unwrap();
    let thread = std::thread::spawn(move || {
        server
            .serve_one(Duration::from_secs(2), |_, action| {
                assert!(matches!(action, request::Action::Execute(_)));
                response::Result::Accepted(Empty {})
            })
            .unwrap();
    });
    assert_eq!(
        client.execute(grant(), [15; 16], [16; 32], Duration::from_secs(2)),
        ConsumptionOutcome::Completed
    );
    thread.join().unwrap();

    let (client, server) = pair();
    let mut client = AuthorityChannel::new(client, peer(), &scope).unwrap();
    let mut server = AuthorityChannel::new(server, peer(), &scope).unwrap();
    let thread = std::thread::spawn(move || {
        let result = server.serve_one_fallible(Duration::from_secs(2), |_, action| {
            assert!(matches!(action, request::Action::Execute(_)));
            // The recipient could perform the action and lose the result.
            Err(io::Error::other("неизвестный исход"))
        });
        assert!(result.is_err());
    });
    assert_eq!(
        client.execute(grant(), [15; 16], [16; 32], Duration::from_secs(2)),
        ConsumptionOutcome::Unknown
    );
    thread.join().unwrap();
}

#[test]
fn round_trip_challenge_and_scoped_authorization_over_seqpacket() {
    let (client, server) = pair();
    let scope = scope();
    let mut client = AuthorityChannel::new(client, peer(), &scope).unwrap();
    let mut server = AuthorityChannel::new(server, peer(), &scope).unwrap();
    let thread = std::thread::spawn(move || {
        for _ in 0..5 {
            server
                .serve_one(Duration::from_secs(2), |_, action| match action {
                    request::Action::Assess(_)
                    | request::Action::Revalidate(_)
                    | request::Action::Revoke(_)
                    | request::Action::Execute(_)
                    | request::Action::BindOwner(_) => response::Result::Accepted(Empty {}),
                    request::Action::BindPlan(_) => response::Result::Accepted(Empty {}),
                    request::Action::PrepareChallenge(_) => response::Result::Challenge(Validate {
                        challenge: vec![1],
                        challenge_ref: vec![15; 16],
                        validity_ms: 500,
                    }),
                    request::Action::Validate(value) => response::Result::Validated(Approved {
                        reference: value.challenge_ref.clone(),
                        challenge_digest: Sha256::digest(&value.challenge).to_vec(),
                        deadline_ns: 5_000,
                        owner_binding: vec![],
                    }),
                    request::Action::Authorize(value) => response::Result::Authorized(Approved {
                        reference: vec![14; 16],
                        challenge_digest: value.challenge_digest.clone(),
                        deadline_ns: 4_000,
                        owner_binding: vec![],
                    }),
                })
                .unwrap();
        }
    });
    assert!(matches!(
        client
            .request(
                grant(),
                request::Action::Assess(Empty {}),
                Duration::from_secs(2)
            )
            .unwrap(),
        response::Result::Accepted(_)
    ));
    let challenge = vec![1, 2, 3];
    let digest = Sha256::digest(&challenge).to_vec();
    let observation = Validate {
        challenge,
        challenge_ref: vec![15; 16],
        validity_ms: 100,
    };
    assert!(matches!(
        client
            .request(
                grant(),
                request::Action::Validate(observation),
                Duration::from_secs(2)
            )
            .unwrap(),
        response::Result::Validated(_)
    ));
    let request = Authorization {
        reference: vec![15; 16],
        challenge_digest: digest.clone(),
    };
    assert!(matches!(
        client
            .request(
                grant(),
                request::Action::Authorize(request),
                Duration::from_secs(2)
            )
            .unwrap(),
        response::Result::Authorized(_)
    ));
    let request = Authorization {
        reference: vec![14; 16],
        challenge_digest: digest,
    };
    assert!(matches!(
        client
            .request(
                grant(),
                request::Action::Revalidate(request),
                Duration::from_secs(2)
            )
            .unwrap(),
        response::Result::Accepted(_)
    ));
    assert!(matches!(
        client
            .request(
                grant(),
                request::Action::Revoke(Empty {}),
                Duration::from_secs(2)
            )
            .unwrap(),
        response::Result::Accepted(_)
    ));
    thread.join().unwrap();
}

#[test]
fn foreign_scope_and_peer_fail_closed() {
    let (a, b) = pair();
    let actual = scope();
    let wrong_peer = ExpectedPeer {
        pid: peer().pid + 1,
        uid: peer().uid,
    };
    assert!(AuthorityChannel::new(a, wrong_peer, &actual).is_err());
    drop(b);
    let (a, b) = pair();
    let mut foreign = actual;
    foreign.uid = 1000;
    let mut server = AuthorityChannel::new(b, peer(), &actual).unwrap();
    let mut client = AuthorityChannel::new(a, peer(), &foreign).unwrap();
    let thread = std::thread::spawn(move || {
        server.serve_one(Duration::from_secs(2), |_, _| panic!("foreign scope"))
    });
    assert!(
        client
            .request(
                grant(),
                request::Action::Assess(Empty {}),
                Duration::from_secs(2)
            )
            .is_err()
    );
    assert!(thread.join().unwrap().is_err());
}

#[test]
fn replay_and_duplicate_field_are_rejected_before_handler() {
    for duplicate in [false, true] {
        let (a, b) = pair();
        let scope = scope();
        let mut server = AuthorityChannel::new(b, peer(), &scope).unwrap();
        let frame = Request {
            version: VERSION,
            sequence: if duplicate { 1 } else { 2 },
            scope: canonical_scope(&scope),
            grant: Some(grant()),
            action: Some(request::Action::Assess(Empty {})),
        };
        let mut bytes = frame.encode_to_vec();
        if duplicate {
            bytes.extend_from_slice(&[0x08, VERSION as u8]);
        }
        // SAFETY: We send an immutable buffer through a socket belonging to the test.
        assert_eq!(
            unsafe {
                libc::send(
                    a.as_raw_fd(),
                    bytes.as_ptr().cast(),
                    bytes.len(),
                    libc::MSG_NOSIGNAL,
                )
            },
            bytes.len() as isize
        );
        assert!(
            server
                .serve_one(Duration::from_secs(2), |_, _| panic!(
                    "bad frame reached handler"
                ))
                .is_err()
        );
    }
}

#[test]
fn separate_control_channel_can_revoke_while_authority_waits() {
    let scope = scope();
    let (op_client, op_server) = pair();
    let (ctrl_client, ctrl_server) = pair();
    let mut op_client = AuthorityChannel::new(op_client, peer(), &scope).unwrap();
    let mut op_server = AuthorityChannel::new(op_server, peer(), &scope).unwrap();
    let mut ctrl_client = AuthorityChannel::new(ctrl_client, peer(), &scope).unwrap();
    let mut ctrl_server = AuthorityChannel::new(ctrl_server, peer(), &scope).unwrap();
    let revoked = Arc::new(AtomicBool::new(false));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let revoked_in_op = revoked.clone();
    let operation = std::thread::spawn(move || {
        op_server.serve_one(Duration::from_secs(3), |_, _| {
            entered_tx.send(()).unwrap();
            resume_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(revoked_in_op.load(Ordering::SeqCst));
            response::Result::Refused(Empty {})
        })
    });
    let request = Authorization {
        reference: vec![15; 16],
        challenge_digest: vec![16; 32],
    };
    let waiting = std::thread::spawn(move || {
        op_client.request(
            grant(),
            request::Action::Authorize(request),
            Duration::from_secs(3),
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let control = std::thread::spawn(move || {
        ctrl_server.serve_one(Duration::from_secs(2), |_, action| {
            assert!(matches!(action, request::Action::Revoke(_)));
            revoked.store(true, Ordering::SeqCst);
            response::Result::Accepted(Empty {})
        })
    });
    assert!(matches!(
        ctrl_client
            .request(
                grant(),
                request::Action::Revoke(Empty {}),
                Duration::from_secs(2)
            )
            .unwrap(),
        response::Result::Accepted(_)
    ));
    resume_tx.send(()).unwrap();
    assert!(matches!(
        waiting.join().unwrap().unwrap(),
        response::Result::Refused(_)
    ));
    operation.join().unwrap().unwrap();
    control.join().unwrap().unwrap();
}
