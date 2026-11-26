use carosign_core::device::DeviceEpoch;
use carosign_core::identity::{OwnerRef, SubjectRef};
use carosign_core::ids::{AuthenticatorId, AuthorityId, BootId, GrantId, OperationId, SubjectId};
use carosign_core::operation::context::{BootDeadline, VerificationPurpose};

use crate::transition::OperationState;

/// Context of the operation for inspections: numbers are invented, but not zero.
pub fn context() -> carosign_core::operation::context::OperationContext {
    carosign_core::operation::context::OperationContext {
        operation_id: OperationId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([2u8; 16]).expect("валид"),
            subject: SubjectId::from_bytes([3u8; 16]).expect("валид"),
        },
        authenticator: carosign_core::device::AuthenticatorRef {
            id: AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
            device_epoch: DeviceEpoch::FIRST,
        },
        owner: OwnerRef {
            authority: AuthorityId::from_bytes([2u8; 16]).expect("валид"),
            binding: GrantId::from_bytes([5u8; 16]).expect("валид"),
        },
        purpose: VerificationPurpose::SessionLogin,
        deadline: BootDeadline::new(BootId::from_bytes([6u8; 16]).expect("валид"), 1_000_000_000),
    }
}

/// Initial status of the operation.
pub fn initial() -> OperationState {
    OperationState::verification(context())
}
