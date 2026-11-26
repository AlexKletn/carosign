use crate::device::AuthenticatorRef;
use crate::identity::OwnerRef;
use crate::identity::SubjectRef;
use crate::ids::BootId;
use crate::ids::OperationId;

use super::*;
use crate::device::DeviceEpoch;
use crate::ids::{AuthenticatorId, AuthorityId, GrantId, SubjectId};

fn boot(byte: u8) -> BootId {
    BootId::from_bytes([byte; 16]).expect("валид")
}

fn context(deadline_ns: u64) -> OperationContext {
    OperationContext {
        operation_id: OperationId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([2u8; 16]).expect("валид"),
            subject: SubjectId::from_bytes([3u8; 16]).expect("валид"),
        },
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
            device_epoch: DeviceEpoch::FIRST,
        },
        owner: OwnerRef {
            authority: AuthorityId::from_bytes([2u8; 16]).expect("валид"),
            binding: GrantId::from_bytes([5u8; 16]).expect("валид"),
        },
        purpose: VerificationPurpose::SessionLogin,
        deadline: BootDeadline::new(boot(6), deadline_ns),
    }
}

#[test]
fn purpose_round_trip_strictly() {
    for (value, purpose) in [
        (1, VerificationPurpose::SessionLogin),
        (2, VerificationPurpose::SessionUnlock),
        (3, VerificationPurpose::PrivilegedAction),
        (4, VerificationPurpose::FactorSelfCheck),
        (5, VerificationPurpose::KeyOperation),
    ] {
        assert_eq!(VerificationPurpose::from_wire(value), Ok(purpose));
    }
    assert!(VerificationPurpose::from_wire(0).is_err());
    assert!(VerificationPurpose::from_wire(6).is_err());
}

#[test]
fn key_operation_requires_hardware_assurance() {
    assert!(VerificationPurpose::KeyOperation.requires_hardware_assurance());
    assert!(!VerificationPurpose::SessionLogin.requires_hardware_assurance());
}

#[test]
fn deadline_is_bound_to_its_boot() {
    let c = context(1_000);
    let same_boot = BootInstant::new(boot(6), 999);
    let reached = BootInstant::new(boot(6), 1_000);
    let other_boot = BootInstant::new(boot(7), 0);
    assert!(!c.deadline.is_expired_at(same_boot));
    assert!(c.deadline.is_expired_at(reached));
    assert!(c.deadline.is_expired_at(other_boot));
}

#[test]
fn deadline_saturates_instead_of_wrapping() {
    let now = BootInstant::new(boot(1), u64::MAX - 1);
    let deadline = now.deadline_after(10);
    assert_eq!(deadline.instant_ns(), u64::MAX);
}
