use carosign_core::device::DeviceEpoch;
use carosign_core::ids::ResourceId;

use super::*;

fn epoch() -> DeviceEpoch {
    DeviceEpoch::FIRST
}

fn delegation(role: Role) -> Delegation {
    Delegation {
        resource: ResourceId::from_bytes([1u8; 16]).expect("валид"),
        role,
        epoch: epoch(),
    }
}

#[test]
fn config_must_be_sealed_and_read_only() {
    assert!(
        may_delegate(
            delegation(Role::Config),
            epoch(),
            HandleKind::SealedMemfd,
            AccessMode {
                read: true,
                write: false,
                directory: false,
                sealed: true,
            }
        )
        .is_ok()
    );
    // Changable setting is no longer a setup.
    assert_eq!(
        may_delegate(
            delegation(Role::Config),
            epoch(),
            HandleKind::SealedMemfd,
            AccessMode::READ_WRITE
        ),
        Err(DelegationError::TooBroad)
    );
    assert_eq!(
        may_delegate(
            delegation(Role::Config),
            epoch(),
            HandleKind::SealedMemfd,
            AccessMode {
                read: true,
                write: true,
                directory: false,
                sealed: true
            }
        ),
        Err(DelegationError::TooBroad)
    );
}

#[test]
fn wrong_kind_is_refused() {
    assert_eq!(
        may_delegate(
            delegation(Role::Device),
            epoch(),
            HandleKind::SocketPair,
            AccessMode::READ_WRITE
        ),
        Err(DelegationError::WrongKind)
    );
}

#[test]
fn stale_epoch_is_refused_before_handoff() {
    let old = Delegation {
        resource: ResourceId::from_bytes([1u8; 16]).expect("валид"),
        role: Role::Device,
        epoch: epoch(),
    };
    assert_eq!(
        may_delegate(
            old,
            epoch().next().expect("не переполнено"),
            HandleKind::CharacterDevice,
            AccessMode::READ_WRITE
        ),
        Err(DelegationError::StaleEpoch)
    );
}

#[test]
fn handed_off_descriptor_is_only_revocable_by_termination() {
    assert_eq!(
        revocation_after_handoff(true),
        Revocation::OnlyByTermination
    );
    assert_eq!(
        revocation_after_handoff(false),
        Revocation::NothingDelegated
    );
}

#[test]
fn negative_fd_is_refused() {
    assert!(OwnedFd::from_raw(-1).is_none());
    assert!(OwnedFd::from_raw(3).is_some());
}
