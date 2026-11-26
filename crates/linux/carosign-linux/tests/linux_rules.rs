//! Linux Layer Rules: Describer Rights, Startup, Sandbox, Death of Process.
//!
//! ⚠️ There's no real kernel call here on purpose: rules are checked on a host without a
//! tablet. `libc` A separate, final step, and its place is marked.

use carosign_core::device::DeviceEpoch;
use carosign_core::ids::ResourceId;
use carosign_linux::process::pidfd::{AfterDeath, ProcessState, WaitBudget};
use carosign_linux::process::sandbox::{Relaxation, Sandbox, SandboxError};
use carosign_linux::process::spawn::{Launch, LaunchStage, SpawnError};
use carosign_linux::process::verified_exec::{LaunchError, Preconditions};
use carosign_linux::resources::fd::{
    AccessMode, Delegation, DelegationError, HandleKind, OwnedFd, Revocation, Role, may_delegate,
    revocation_after_handoff,
};

fn delegation(role: Role) -> Delegation {
    Delegation {
        resource: ResourceId::from_bytes([1u8; 16]).expect("валид"),
        role,
        epoch: DeviceEpoch::FIRST,
    }
}

#[test]
fn sealed_config_is_read_only() {
    let sealed = AccessMode {
        read: true,
        write: false,
        directory: false,
        sealed: true,
    };
    assert!(
        may_delegate(
            delegation(Role::Config),
            DeviceEpoch::FIRST,
            HandleKind::SealedMemfd,
            sealed
        )
        .is_ok()
    );
    assert_eq!(
        may_delegate(
            delegation(Role::Config),
            DeviceEpoch::FIRST,
            HandleKind::SealedMemfd,
            AccessMode::READ_WRITE
        ),
        Err(DelegationError::TooBroad)
    );
}

#[test]
fn stale_epoch_blocks_handoff() {
    let next = DeviceEpoch::FIRST.next().expect("не переполнено");
    assert_eq!(
        may_delegate(
            delegation(Role::Device),
            next,
            HandleKind::CharacterDevice,
            AccessMode::READ_WRITE
        ),
        Err(DelegationError::StaleEpoch)
    );
}

#[test]
fn handed_off_fd_is_revocable_only_by_termination() {
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
    assert_eq!(
        format!("{:?}", OwnedFd::from_raw(5).unwrap()),
        "OwnedFd(..)"
    );
}

#[test]
fn launch_requires_all_conditions() {
    let mut launch = Launch::new();
    let bad = Preconditions {
        trusted_root: true,
        regular_file: false,
        root_owned: true,
        not_world_writable: true,
        manifest_ok: true,
    };
    assert_eq!(
        launch.check(&bad, &Sandbox::default(), &[]),
        Err(SpawnError::Launch(LaunchError::NotRegularFile))
    );
    assert_eq!(launch.stage(), LaunchStage::Idle);
}

#[test]
fn root_is_never_granted() {
    let sandbox = Sandbox {
        root: true,
        ..Sandbox::default()
    };
    assert_eq!(
        sandbox.check(&[Relaxation {
            field: "root",
            reason: "языку нужен".into()
        }]),
        Err(SandboxError::RootForbidden)
    );
}

#[test]
fn network_needs_a_documented_relaxation() {
    let sandbox = Sandbox {
        network: true,
        ..Sandbox::default()
    };
    assert!(sandbox.check(&[]).is_err());
    assert!(
        sandbox
            .check(&[Relaxation {
                field: "network",
                reason: "разрешено владельцем 11.11".into()
            }])
            .is_ok()
    );
}

#[test]
fn death_without_kernel_evidence_leaves_access_uncertain() {
    assert_eq!(
        carosign_linux::process::pidfd::after_death(ProcessState::Exited { code: Some(0) }, false),
        AfterDeath::AccessMayRemain
    );
    assert_eq!(
        carosign_linux::process::pidfd::after_death(ProcessState::Exited { code: Some(0) }, true),
        AfterDeath::AccessRevokedByKernel
    );
    // The living process doesn't prove anything either.
    assert_eq!(
        carosign_linux::process::pidfd::after_death(ProcessState::Running, true),
        AfterDeath::AccessMayRemain
    );
}

#[test]
fn wait_timeout_is_unknown_not_assumed() {
    let budget = WaitBudget::cleanup();
    assert_eq!(budget.timeout_ms, 10_000);
    assert_eq!(budget.outcome(None), ProcessState::Unknown);
}
