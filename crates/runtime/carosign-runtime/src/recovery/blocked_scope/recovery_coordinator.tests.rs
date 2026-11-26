use super::super::*;
use super::*;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, MutationId};
use carosign_core::ids::{AuthorityId, SubjectId};

#[test]
fn blocked_scopes_are_bounded_and_cannot_share_a_mutation() {
    let mut coordinator = RecoveryCoordinator::new();
    let authenticator = AuthenticatorId::from_bytes([1; 16]).unwrap();
    let authority = AuthorityId::from_bytes([2; 16]).unwrap();
    for number in 1..=RECOVERY_SCOPES_MAX as u8 {
        let scope = RecoveryScope {
            subject: SubjectRef {
                authority,
                subject: SubjectId::from_bytes([number; 16]).unwrap(),
            },
            authenticator,
        };
        coordinator
            .block(scope, MutationId::from_bytes([number; 16]).unwrap())
            .unwrap();
        assert!(coordinator.is_blocked(scope));
    }
    let extra = RecoveryScope {
        subject: SubjectRef {
            authority,
            subject: SubjectId::from_bytes([100; 16]).unwrap(),
        },
        authenticator,
    };
    assert_eq!(
        coordinator.block(extra, MutationId::from_bytes([1; 16]).unwrap()),
        Err(RecoveryError::Conflict)
    );
    assert_eq!(
        coordinator.block(extra, MutationId::from_bytes([100; 16]).unwrap()),
        Err(RecoveryError::Limit)
    );
    assert!(!coordinator.is_blocked(extra));
}
