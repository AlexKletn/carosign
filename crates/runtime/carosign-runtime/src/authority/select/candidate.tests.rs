use super::super::*;
use super::*;
use crate::operation::bindings::{BindingKind, SetError};
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::EnrollmentGeneration;
use carosign_core::enrollment::EnrollmentRef;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::AuthenticatorId;
use carosign_core::ids::{AuthorityId, EnrollmentId, SubjectId};

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn authenticator() -> AuthenticatorId {
    AuthenticatorId::from_bytes([3u8; 16]).expect("валид")
}

fn candidate(byte: u8, active: bool) -> Candidate {
    let status = if active {
        Status::Active
    } else {
        Status::Unavailable
    };
    Candidate {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
            authenticator: authenticator(),
            generation: EnrollmentGeneration::FIRST,
        },
        status,
        custody: Custody::Provider,
        provider_ref: vec![byte; 4],
        format_id: Some("goodix-asp".into()),
        format_version: 1,
    }
}

#[test]
fn no_active_enrollment_is_not_enrolled_not_scan_all() {
    // ⚠️ D-03: "No registrations" - rejection, not "look for someone."
    let result = select(
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
        &[candidate(1, false), candidate(2, false)],
    );
    assert_eq!(result, Err(SelectionError::NotEnrolled));
}

#[test]
fn only_active_ones_are_included() {
    let set = select(
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
        &[candidate(1, true), candidate(2, false), candidate(3, true)],
    )
    .expect("набор");
    assert_eq!(set.len(), 2);
    assert!(set.contains(candidate(1, true).enrollment));
    assert!(!set.contains(candidate(2, true).enrollment));
}

#[test]
fn foreign_authenticator_is_corruption_not_widening() {
    let mut foreign = candidate(1, true);
    foreign.enrollment.authenticator = AuthenticatorId::from_bytes([9u8; 16]).expect("валид");
    assert_eq!(
        select(subject(), authenticator(), DeviceEpoch::FIRST, &[foreign]),
        Err(SelectionError::WrongAuthenticator)
    );
}

#[test]
fn vault_binding_rejects_foreign_bytes() {
    let mut vault = candidate(4, true);
    vault.custody = Custody::PlatformVault;
    vault.provider_ref = vec![9, 9];
    assert_eq!(
        select(
            subject(),
            authenticator(),
            DeviceEpoch::FIRST,
            &[vault.clone()]
        ),
        Err(SelectionError::Set(SetError::Binding(
            crate::operation::bindings::BindingError::KindMismatch
        )))
    );
    vault.provider_ref.clear();
    let set = select(subject(), authenticator(), DeviceEpoch::FIRST, &[vault]).expect("набор");
    assert!(set.bindings()[0].provider_ref.is_empty());
    assert_eq!(set.bindings()[0].kind, BindingKind::ScopedVault);
}
