use super::*;
use carosign_core::{
    authorization::GrantScope,
    device::DeviceEpoch,
    identity::SubjectRef,
    ids::{
        AuthenticatorId, AuthorityId, BootId, CommandId, GrantId, LeaseId, MutationId, OperationId,
        SubjectId, WorkerInstanceId,
    },
};
use carosign_linux_ipc::enrollment_authority::{Empty, request, response};

#[test]
fn connector_binds_two_endpoints_to_same_scope_and_grant() {
    let scope = EnrollmentAuthorizationScope {
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
    };
    let grant = ValidatedGrant::issue_for_port(
        GrantId::from_bytes([12; 16]).unwrap(),
        AuthorityId::from_bytes([13; 16]).unwrap(),
        GrantScope::Enroll {
            subject: scope.subject,
            authenticator: scope.authenticator.id,
            replace: None,
        },
        scope.boot,
        1000,
    );
    let op = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let control = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let connector = OwnerConnector::new(
        scope.authenticator,
        scope.recipient,
        op.platform,
        control.platform,
    );
    let peer = ExpectedPeer {
        pid: std::process::id(),
        uid: unsafe { libc::geteuid() },
    };
    let expected = GrantBinding::from_grant(&grant);
    let workers: Vec<_> = [op.worker, control.worker]
        .into_iter()
        .map(|fd| {
            let expected = expected.clone();
            std::thread::spawn(move || {
                let (mut server, actual_scope, actual_grant, payload) =
                    AuthorityChannel::bootstrap_server(fd, peer, Duration::from_secs(2)).unwrap();
                assert_eq!(actual_scope, scope);
                assert!(actual_grant == expected && payload.is_empty());
                server
                    .serve_one(Duration::from_secs(2), |_, action| {
                        assert!(matches!(
                            action,
                            request::Action::Assess(_) | request::Action::Revoke(_)
                        ));
                        response::Result::Accepted(Empty {})
                    })
                    .unwrap();
            })
        })
        .collect();
    let (mut operation, mut control) = connector.connect(&scope, &grant).unwrap();
    assert!(connector.connect(&scope, &grant).is_err());
    assert!(matches!(
        operation.request(
            expected.clone(),
            request::Action::Assess(Empty {}),
            Duration::from_secs(2)
        ),
        Ok(response::Result::Accepted(_))
    ));
    assert!(matches!(
        control.request(
            expected,
            request::Action::Revoke(Empty {}),
            Duration::from_secs(2)
        ),
        Ok(response::Result::Accepted(_))
    ));
    for worker in workers {
        worker.join().unwrap();
    }
}
