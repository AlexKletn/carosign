use super::super::cgroup::CgroupError;
use super::super::cgroup::PreparedCgroup;
use super::super::gate::ErrorWriter;
use super::super::gate::GateReceiver;
use super::super::namespaces;
use super::super::tmpfs_root;
use super::HelperStage;
use super::OwnedExecution;
use super::OwnedRuntimeFile;
use super::RuntimeFile;
use super::RuntimeIdentity;
use super::execute_prepared::execute_prepared;
use super::prepare_root_with;
use super::terminate;
use std::num::NonZeroU64;
use std::os::fd::AsFd;
use std::time::Duration;

/// Non-returning path of a single-threaded individual helper: The composition must confirm
/// the installation, policy and affiliation of all transmitted FDs.
/// Prior to the call, the process should not have other living owners of the descriptors.
#[allow(clippy::too_many_arguments)]
pub(crate) fn enter_owned(
    group: PreparedCgroup,
    gate: GateReceiver,
    report: ErrorWriter,
    timeout: Duration,
    root_size: NonZeroU64,
    identity: RuntimeIdentity,
    runtime: Vec<OwnedRuntimeFile>,
    execution: OwnedExecution,
) -> ! {
    if let Err(error) = group.join_current_and_wait(gate, timeout) {
        let (stage, errno) = match error {
            CgroupError::Gate(error) => {
                (HelperStage::Gate, error.raw_os_error().unwrap_or(libc::EIO))
            }
            CgroupError::Kernel { source, .. } => (
                HelperStage::Cgroup,
                source.raw_os_error().unwrap_or(libc::EIO),
            ),
            _ => (HelperStage::Cgroup, libc::EINVAL),
        };
        terminate(report, stage, errno);
    }
    // After joining, the group does not need descriptors inside the assistant.
    drop(group);
    after_gate_owned(report, root_size, identity, runtime, execution)
}

/// The remainder of the same non-return path after cgroup and gate.
/// The input is separately available only within the process module for testing without a
/// delegated cgroup; it does not, by itself, confirm its installation.
pub(in crate::process) fn after_gate_owned(
    report: ErrorWriter,
    root_size: NonZeroU64,
    identity: RuntimeIdentity,
    runtime: Vec<OwnedRuntimeFile>,
    execution: OwnedExecution,
) -> ! {
    let borrowed: Vec<_> = runtime
        .iter()
        .map(|file| RuntimeFile {
            path: &file.path,
            source: file.source.as_fd(),
        })
        .collect();
    let root = match prepare_root_with(
        root_size,
        identity,
        &borrowed,
        namespaces::enter,
        tmpfs_root::create,
    ) {
        Ok(root) => root,
        Err((stage, errno)) => terminate(report, stage, errno),
    };
    drop(borrowed);
    drop(runtime);
    execute_prepared(root, report, identity, execution)
}
