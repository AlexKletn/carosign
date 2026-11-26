use super::*;
use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::device::AuthenticatorRef;
use carosign_core::device::DeviceEpoch;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, SubjectId};
use carosign_core::ids::{
    AuthorityId, BootId, CommandId, GrantId, LeaseId, MutationId, OperationId, WorkerInstanceId,
};
use carosign_core::operation::context::BootInstant;
use futures_executor::block_on;

fn fixture() -> (EnrollmentAuthorizationScope, ValidatedGrant, BootInstant) {
    let boot = BootId::from_bytes([1; 16]).unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([2; 16]).unwrap(),
        subject: SubjectId::from_bytes([3; 16]).unwrap(),
    };
    let authenticator = AuthenticatorRef {
        id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
        device_epoch: DeviceEpoch::FIRST,
    };
    let scope = EnrollmentAuthorizationScope {
        operation: OperationId::from_bytes([5; 16]).unwrap(),
        mutation: MutationId::from_bytes([6; 16]).unwrap(),
        challenge_command: CommandId::from_bytes([7; 16]).unwrap(),
        command: CommandId::from_bytes([8; 16]).unwrap(),
        subject,
        uid: 0,
        authenticator,
        recipient: EnrollmentRecipientId::from_bytes([32; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([9; 16]).unwrap(),
        lease: LeaseId::from_bytes([10; 16]).unwrap(),
        boot,
    };
    let grant = ValidatedGrant::issue_for_port(
        GrantId::from_bytes([11; 16]).unwrap(),
        AuthorityId::from_bytes([12; 16]).unwrap(),
        GrantScope::Enroll {
            subject,
            authenticator: authenticator.id,
            replace: None,
        },
        boot,
        2_000_000_000,
    );
    (scope, grant, BootInstant::new(boot, 1_000_000_000))
}

#[test]
fn observation_rejects_bad_shape() {
    for (challenge, reference, duration, max) in [
        (vec![], [1; 16], 1, 5),
        (vec![1; 65], [1; 16], 1, 5),
        (vec![1], [0; 16], 1, 5),
        (vec![1], [1; 16], 0, 5),
        (vec![1], [1; 16], 6, 5),
        (vec![1], [1; 16], u64::MAX, u64::MAX),
    ] {
        assert!(ChallengeObservation::new(challenge, reference, duration, max).is_err());
    }
}

#[test]
fn scope_challenge_and_expiry_are_bound() {
    let (scope, grant, now) = fixture();
    let observation = ChallengeObservation::new(vec![1, 2, 3], [13; 16], 500, 500).unwrap();
    let deadline = BootInstant::new(scope.boot, 1_500_000_000);
    let challenge =
        ValidatedChallenge::issued_for_port(scope, &observation, &grant, now, deadline).unwrap();
    assert!(challenge.check(&scope, &grant, now).is_ok());
    assert!(challenge.check_observation(&observation).is_ok());
    let other = ChallengeObservation::new(vec![1, 2, 4], [13; 16], 500, 500).unwrap();
    assert_eq!(
        challenge.check_observation(&other),
        Err(EnrollmentAuthorizationError::Invalid)
    );
    let mut foreign = scope;
    foreign.uid = 1000;
    assert_eq!(
        challenge.check(&foreign, &grant, now),
        Err(EnrollmentAuthorizationError::Invalid)
    );
    let mut foreign_recipient = scope;
    foreign_recipient.recipient = EnrollmentRecipientId::from_bytes([33; 16]).unwrap();
    assert_eq!(
        challenge.check(&foreign_recipient, &grant, now),
        Err(EnrollmentAuthorizationError::Invalid)
    );
    assert!(EnrollmentRecipientId::from_bytes([0; 16]).is_err());
    assert_eq!(
        challenge.check(&scope, &grant, deadline),
        Err(EnrollmentAuthorizationError::Expired)
    );
    assert_eq!(
        challenge.check(&scope, &grant, BootInstant::new(scope.boot, 999_999_999)),
        Err(EnrollmentAuthorizationError::Invalid)
    );
    assert_eq!(
        challenge.check(
            &scope,
            &grant,
            BootInstant::new(BootId::from_bytes([31; 16]).unwrap(), 1)
        ),
        Err(EnrollmentAuthorizationError::Invalid)
    );
    let authorization = AuthorizedEnrollment::issued_for_port(&challenge, [14; 16]).unwrap();
    assert!(authorization.check(&scope, &grant, now).is_ok());
    assert_eq!(
        authorization.check(&foreign_recipient, &grant, now),
        Err(EnrollmentAuthorizationError::Invalid)
    );
    assert_eq!(authorization.digest(), &observation.digest());
    assert!(AuthorizedEnrollment::issued_for_port(&challenge, [0; 16]).is_err());
}

#[test]
fn unavailable_source_refuses_before_preparation() {
    let (scope, grant, _) = fixture();
    assert_eq!(
        UnavailableEnrollmentAuthority.recipient_for(scope.authenticator),
        Err(EnrollmentAuthorizationError::Unavailable)
    );
    assert_eq!(
        block_on(UnavailableEnrollmentAuthority.assess_preparation(&scope, &grant)),
        Err(EnrollmentAuthorizationError::Unavailable)
    );
    assert_eq!(
        block_on(UnavailableEnrollmentAuthority.invalidate(&scope, None, None)),
        Err(EnrollmentAuthorizationError::Unavailable)
    );
}
