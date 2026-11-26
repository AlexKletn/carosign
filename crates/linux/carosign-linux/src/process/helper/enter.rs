use super::super::cgroup::CgroupError;
use super::super::cgroup::PreparedCgroup;
use super::super::gate::ErrorWriter;
use super::super::gate::GateReceiver;
use super::super::namespaces;
use super::super::tmpfs_root;
use super::HelperFailure;
use super::HelperStage;
use super::RuntimeFile;
use super::RuntimeIdentity;
use super::fail;
use super::prepare_root_with;
use std::num::NonZeroU64;
use std::os::fd::OwnedFd;
use std::time::Duration;

/// Performs testable initial steps and does not run the program.
/// Called only in a separate single-threaded assistant, not after fork multithreaded service.
/// The parent is obliged to limit its lifespan and collect.
pub(crate) fn enter(
    group: &PreparedCgroup,
    gate: GateReceiver,
    report: ErrorWriter,
    timeout: Duration,
    root_size: NonZeroU64,
    identity: RuntimeIdentity,
    runtime: &[RuntimeFile<'_>],
) -> HelperFailure {
    let (stage, errno) = match group.join_current_and_wait(gate, timeout) {
        Ok(()) => return after_gate(report, root_size, identity, runtime),
        Err(CgroupError::Gate(error)) => {
            (HelperStage::Gate, error.raw_os_error().unwrap_or(libc::EIO))
        }
        Err(CgroupError::Kernel { source, .. }) => (
            HelperStage::Cgroup,
            source.raw_os_error().unwrap_or(libc::EIO),
        ),
        Err(_) => (HelperStage::Cgroup, libc::EINVAL),
    };
    fail(report, stage, errno)
}

/// The following barriers are performed only in the single-threaded assistant after the gate.
/// Absent restrictions and origin cannot be replaced by no_new_privs Or creating namespaces.
pub(super) fn after_gate(
    report: ErrorWriter,
    root_size: NonZeroU64,
    identity: RuntimeIdentity,
    runtime: &[RuntimeFile<'_>],
) -> HelperFailure {
    after_gate_with(
        report,
        root_size,
        identity,
        runtime,
        namespaces::enter,
        tmpfs_root::create,
    )
}

pub(super) fn after_gate_with(
    report: ErrorWriter,
    root_size: NonZeroU64,
    identity: RuntimeIdentity,
    runtime: &[RuntimeFile<'_>],
    enter_namespaces: impl FnOnce() -> Result<(), namespaces::NamespaceError>,
    create_root: impl FnOnce(NonZeroU64) -> Result<OwnedFd, tmpfs_root::RootError>,
) -> HelperFailure {
    match prepare_root_with(root_size, identity, runtime, enter_namespaces, create_root) {
        Ok(_root) => fail(report, HelperStage::Isolation, libc::ENOSYS),
        Err((stage, errno)) => fail(report, stage, errno),
    }
}
