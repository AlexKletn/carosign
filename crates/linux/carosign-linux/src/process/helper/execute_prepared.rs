use std::os::fd::{AsFd, AsRawFd, OwnedFd};

use super::super::{fd_cleanup, gate::ErrorWriter, pinned_exec, privileges, root_switch, seccomp};
use super::{HelperStage, OwnedExecution, RuntimeIdentity, terminate};

/// Called only after all previous steps of the helper.
pub(super) fn execute_prepared(
    root: OwnedFd,
    report: ErrorWriter,
    identity: RuntimeIdentity,
    execution: OwnedExecution,
) -> ! {
    use fd_cleanup::{AllowedFd, ExecDisposition};
    let std_ok = execution.standard.iter().enumerate().all(|(index, fd)| {
        if fd.as_raw_fd() != index as i32 {
            return false;
        }
        // SAFETY: The kernel fills the stat for open FD.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        (unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) == 0 })
            && stat.st_mode & libc::S_IFMT == libc::S_IFCHR
            && stat.st_rdev == libc::makedev(1, 3)
    });
    let required = [
        libc::SYS_write,
        libc::SYS_exit_group,
        libc::SYS_exit,
        libc::SYS_execveat,
        libc::SYS_fstat,
        libc::SYS_pread64,
    ];
    if !std_ok
        || required
            .iter()
            .any(|call| !execution.allowed_syscalls.contains(call))
    {
        terminate(report, HelperStage::Isolation, libc::EINVAL);
    }
    let mut allowed: Vec<_> = execution
        .standard
        .iter()
        .map(|fd| AllowedFd {
            fd: fd.as_raw_fd(),
            disposition: ExecDisposition::Inherit,
        })
        .collect();
    allowed.push(AllowedFd {
        fd: execution.worker.as_raw_fd(),
        disposition: ExecDisposition::Inherit,
    });
    allowed.push(AllowedFd {
        fd: execution.executable.as_raw_fd(),
        disposition: ExecDisposition::Close,
    });
    allowed.push(AllowedFd {
        fd: report.as_raw_fd(),
        disposition: ExecDisposition::Close,
    });
    if let Err(error) = root_switch::enter_prepared(root, &allowed) {
        terminate(
            report,
            HelperStage::RootSwitch,
            error.source.raw_os_error().unwrap_or(libc::EIO),
        );
    }
    if let Err(error) = privileges::drop_identity(identity.uid, identity.gid) {
        terminate(
            report,
            HelperStage::Identity,
            error.source.raw_os_error().unwrap_or(libc::EIO),
        );
    }
    if let Err(error) = seccomp::install(&execution.allowed_syscalls) {
        let errno = match error {
            seccomp::FilterError::Kernel(error) => error.raw_os_error().unwrap_or(libc::EIO),
            _ => libc::EINVAL,
        };
        terminate(report, HelperStage::Seccomp, errno);
    }
    let error = pinned_exec::execute(execution.executable.as_fd(), &execution.args)
        .expect_err("успешный exec не возвращается");
    terminate(
        report,
        HelperStage::Exec,
        error.raw_os_error().unwrap_or(libc::EIO),
    );
}
