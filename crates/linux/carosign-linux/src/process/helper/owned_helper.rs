use super::super::cgroup::PreparedCgroup;
use super::super::fd_cleanup;
use super::super::gate::ErrorWriter;
use super::super::gate::GateReceiver;
use super::HelperStage;
use super::OwnedExecution;
use super::OwnedRuntimeFile;
use super::RuntimeIdentity;
use super::enter_owned;
use super::terminate;
use std::num::NonZeroU64;
use std::os::fd::AsRawFd;
use std::time::Duration;

/// Single-use configuration for a separate helper. Before calling `run`, the process owner
/// verifies the helper’s provenance, single-threaded execution, and lifetime control.
/// The composition itself does not open the production path of the launch.
pub(crate) struct OwnedHelper {
    pub(crate) group: PreparedCgroup,
    pub(crate) gate: GateReceiver,
    pub(crate) report: ErrorWriter,
    pub(crate) timeout: Duration,
    pub(crate) root_size: NonZeroU64,
    pub(crate) identity: RuntimeIdentity,
    pub(crate) runtime: Vec<OwnedRuntimeFile>,
    pub(crate) execution: OwnedExecution,
}

impl OwnedHelper {
    pub(crate) fn run(self) -> ! {
        // The assistant has to be a separate single-threaded process, and before entering the
        // group and waiting for the gate, we close everything that is not part of it.
        // All FDs left until the next clear cleaning are closed at exec.
        let mut fds = Vec::new();
        let mut keep = |fd| {
            fds.push(fd_cleanup::AllowedFd {
                fd,
                disposition: fd_cleanup::ExecDisposition::Close,
            })
        };
        for fd in self.group.helper_fds() {
            keep(fd);
        }
        keep(self.gate.as_raw_fd());
        keep(self.report.as_raw_fd());
        for file in &self.runtime {
            keep(file.source.as_raw_fd());
        }
        keep(self.execution.executable.as_raw_fd());
        keep(self.execution.worker.as_raw_fd());
        for fd in &self.execution.standard {
            keep(fd.as_raw_fd());
        }
        if let Err(error) = fd_cleanup::retain_only(&fds) {
            terminate(
                self.report,
                HelperStage::Isolation,
                error.raw_os_error().unwrap_or(libc::EIO),
            );
        }
        enter_owned(
            self.group,
            self.gate,
            self.report,
            self.timeout,
            self.root_size,
            self.identity,
            self.runtime,
            self.execution,
        )
    }
}
