use crate::enrollment_authority::AuthorityChannel;

use super::*;
use carosign_core::device::DeviceEpoch;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, BootId, CommandId, GrantId, LeaseId, MutationId, OperationId,
    SubjectId, WorkerInstanceId,
};
use carosign_runtime::operation::registry::ManualClock;
use futures_executor::block_on;
use sha2::{Digest, Sha256};
use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::atomic::AtomicUsize;

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

fn grant(scope: &EnrollmentAuthorizationScope) -> ValidatedGrant {
    ValidatedGrant::issue_for_port(
        GrantId::from_bytes([12; 16]).unwrap(),
        AuthorityId::from_bytes([13; 16]).unwrap(),
        GrantScope::Enroll {
            subject: scope.subject,
            authenticator: scope.authenticator.id,
            replace: None,
        },
        scope.boot,
        2_000_000_000,
    )
}

fn pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [-1; 2];
    // SAFETY: Two slots for created descriptors.
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
    // SAFETY: The result of a successful socketpair belongs to the test.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn peer() -> wire::ExpectedPeer {
    // SAFETY: Getpid/geteuid has no preconditions.
    unsafe {
        wire::ExpectedPeer {
            pid: libc::getpid() as u32,
            uid: libc::geteuid(),
        }
    }
}

struct MockConnector {
    recipient: EnrollmentRecipientId,
    revoked: Arc<AtomicUsize>,
}
impl TrustedEnrollmentConnector for MockConnector {
    fn recipient_for(&self, _: AuthenticatorRef) -> Result<EnrollmentRecipientId, Error> {
        Ok(self.recipient)
    }
    fn connect(
        &self,
        scope: &EnrollmentAuthorizationScope,
        _: &ValidatedGrant,
    ) -> Result<(AuthorityChannel, AuthorityChannel), Error> {
        let (operation, server_operation) = pair();
        let (control, server_control) = pair();
        let mut server_operation = AuthorityChannel::new(server_operation, peer(), scope).unwrap();
        let mut server_control = AuthorityChannel::new(server_control, peer(), scope).unwrap();
        let revoked = self.revoked.clone();
        std::thread::spawn(move || {
            for _ in 0..9 {
                if server_operation
                    .serve_one(Duration::from_secs(3), |_, action| match action {
                        wire::request::Action::Assess(_)
                        | wire::request::Action::Revalidate(_)
                        | wire::request::Action::BindOwner(_)
                        | wire::request::Action::BindPlan(_)
                        | wire::request::Action::Execute(_) => {
                            wire::response::Result::Accepted(wire::Empty {})
                        }
                        wire::request::Action::PrepareChallenge(_) => {
                            wire::response::Result::Challenge(wire::Validate {
                                challenge: vec![1, 2, 3],
                                challenge_ref: vec![15; 16],
                                validity_ms: 500,
                            })
                        }
                        wire::request::Action::Validate(value) => {
                            wire::response::Result::Validated(wire::Approved {
                                reference: value.challenge_ref.clone(),
                                challenge_digest: Sha256::digest(&value.challenge).to_vec(),
                                deadline_ns: 1_400_000_000,
                                owner_binding: vec![],
                            })
                        }
                        wire::request::Action::Authorize(value) => {
                            wire::response::Result::Authorized(wire::Approved {
                                reference: vec![14; 16],
                                challenge_digest: value.challenge_digest.clone(),
                                deadline_ns: 1_300_000_000,
                                owner_binding: b"owner binding".to_vec(),
                            })
                        }
                        _ => wire::response::Result::Refused(wire::Empty {}),
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            let _ = server_control.serve_one(Duration::from_secs(3), |_, action| {
                assert!(matches!(action, wire::request::Action::Revoke(_)));
                revoked.fetch_add(1, Ordering::SeqCst);
                wire::response::Result::Accepted(wire::Empty {})
            });
        });
        Ok((
            AuthorityChannel::new(operation, peer(), scope).unwrap(),
            AuthorityChannel::new(control, peer(), scope).unwrap(),
        ))
    }
}

#[test]
fn trusted_connector_runs_scoped_authorization_and_revoke() {
    let scope = scope();
    let grant = grant(&scope);
    let revoked = Arc::new(AtomicUsize::new(0));
    let clock = Arc::new(ManualClock::new(scope.boot));
    clock.advance(1_000_000_000);
    let authority = IpcEnrollmentAuthority::new(
        Arc::new(MockConnector {
            recipient: scope.recipient,
            revoked: revoked.clone(),
        }),
        clock.clone(),
        Duration::from_secs(2),
        Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(
        authority.recipient_for(scope.authenticator).unwrap(),
        scope.recipient
    );
    block_on(authority.assess_preparation(&scope, &grant)).unwrap();
    let observation = authority
        .prepare_owner_challenge(&scope, &grant, &mut |send| send())
        .unwrap();
    assert!(
        authority
            .prepare_owner_challenge(&scope, &grant, &mut |send| send())
            .is_err()
    );
    let plan = carosign_runtime::enrollment_authorization::EnrollmentPlanBinding {
        planned: carosign_core::enrollment::EnrollmentRef {
            id: carosign_core::ids::EnrollmentId::from_bytes([21; 16]).unwrap(),
            authenticator: scope.authenticator.id,
            generation: carosign_core::enrollment::EnrollmentGeneration::FIRST,
        },
        digest: [22; 32],
        change: carosign_runtime::enrollment_authorization::EnrollmentPlanChange::Create,
    };
    let forged = carosign_runtime::enrollment_authorization::EnrollmentPlanBinding {
        planned: plan.planned,
        digest: plan.digest,
        change: carosign_runtime::enrollment_authorization::EnrollmentPlanChange::Replace {
            old: carosign_core::enrollment::EnrollmentRef {
                id: carosign_core::ids::EnrollmentId::from_bytes([24; 16]).unwrap(),
                ..plan.planned
            },
        },
    };
    assert!(matches!(
        authority.bind_plan(&scope, &grant, &forged),
        Err(Error::Invalid)
    ));
    authority.bind_plan(&scope, &grant, &plan).unwrap();
    let foreign_plan = carosign_runtime::enrollment_authorization::EnrollmentPlanBinding {
        planned: plan.planned,
        digest: [23; 32],
        change: carosign_runtime::enrollment_authorization::EnrollmentPlanChange::Create,
    };
    assert!(authority.bind_plan(&scope, &grant, &foreign_plan).is_err());
    let foreign_challenge = ChallengeObservation::new(vec![9], [15; 16], 500, 500).unwrap();
    assert!(matches!(
        block_on(authority.validate_session(&scope, &foreign_challenge, &grant)),
        Err(Error::Invalid)
    ));
    let validated = block_on(authority.validate_session(&scope, &observation, &grant)).unwrap();
    let authorized = block_on(authority.authorize(&validated, &grant)).unwrap();
    assert_eq!(*authorized.reference(), [14; 16]);
    let binding = authority
        .take_owner_binding(&authorized, &scope, &grant)
        .unwrap();
    assert_eq!(binding, b"owner binding");
    assert!(
        authority
            .take_owner_binding(&authorized, &scope, &grant)
            .is_err()
    );
    block_on(authority.revalidate(&authorized, &scope, &grant)).unwrap();
    authority
        .bind_owner(&authorized, &scope, &grant, binding)
        .unwrap();
    block_on(authority.revalidate(&authorized, &scope, &grant)).unwrap();
    assert_eq!(
        authority.execute_owner(&authorized, &scope, &grant, &mut |send| send()),
        carosign_runtime::enrollment_admission::ConsumptionOutcome::Completed
    );
    assert_eq!(
        authority.execute_owner(&authorized, &scope, &grant, &mut |send| send()),
        carosign_runtime::enrollment_admission::ConsumptionOutcome::NotDispatched
    );
    authority.revoke_now(&scope).unwrap();
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
    assert!(matches!(
        block_on(authority.revalidate(&authorized, &scope, &grant)),
        Err(Error::Revoked)
    ));
    assert!(authority.revoke_now(&scope).is_ok());
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
}

#[test]
fn same_grant_identity_cannot_change_enrollment_scope() {
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
    use carosign_core::ids::EnrollmentId;
    let scope = scope();
    let original = grant(&scope);
    let changed = ValidatedGrant::issue_for_port(
        original.id(),
        original.issuer(),
        GrantScope::Enroll {
            subject: scope.subject,
            authenticator: scope.authenticator.id,
            replace: Some(EnrollmentRef {
                id: EnrollmentId::from_bytes([21; 16]).unwrap(),
                authenticator: scope.authenticator.id,
                generation: EnrollmentGeneration::FIRST,
            }),
        },
        scope.boot,
        original.deadline_ns(),
    );
    let clock = Arc::new(ManualClock::new(scope.boot));
    clock.advance(1_000_000_000);
    let authority = IpcEnrollmentAuthority::new(
        Arc::new(MockConnector {
            recipient: scope.recipient,
            revoked: Arc::new(AtomicUsize::new(0)),
        }),
        clock,
        Duration::from_secs(2),
        Duration::from_secs(1),
    )
    .unwrap();
    block_on(authority.assess_preparation(&scope, &original)).unwrap();
    let observation = ChallengeObservation::new(vec![1, 2, 3], [15; 16], 500, 500).unwrap();
    assert!(matches!(
        block_on(authority.validate_session(&scope, &observation, &changed)),
        Err(Error::Invalid)
    ));
}

#[test]
fn foreign_recipient_is_rejected_before_opening_channels() {
    let scope = scope();
    let grant = grant(&scope);
    let clock = Arc::new(ManualClock::new(scope.boot));
    let authority = IpcEnrollmentAuthority::new(
        Arc::new(MockConnector {
            recipient: EnrollmentRecipientId::from_bytes([22; 16]).unwrap(),
            revoked: Arc::new(AtomicUsize::new(0)),
        }),
        clock,
        Duration::from_secs(2),
        Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(
        block_on(authority.assess_preparation(&scope, &grant)),
        Err(Error::Invalid)
    );
}
