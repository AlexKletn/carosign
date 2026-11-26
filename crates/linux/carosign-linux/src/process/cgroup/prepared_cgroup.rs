use super::CgroupError;
use super::Limits;
use std::ffi::CStr;
use std::ffi::CString;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

const CGROUP2_SUPER_MAGIC: libc::c_long = 0x6367_7270;

/// The created group holds the parent's descriptor to remove the empty group.
pub(crate) struct PreparedCgroup {
    pub(super) parent: OwnedFd,
    pub(super) directory: OwnedFd,
    pub(super) name: CString,
}

pub(super) fn errno(stage: &'static str) -> CgroupError {
    CgroupError::Kernel {
        stage,
        source: io::Error::last_os_error(),
    }
}

fn duplicate(fd: BorrowedFd<'_>) -> Result<OwnedFd, CgroupError> {
    // SAFETY: fd is valid; a successful fcntl returns a new descriptor.
    let result = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if result < 0 {
        return Err(errno("duplicate_root"));
    }
    // SAFETY: The result is a newly created descriptor with a single owner.
    Ok(unsafe { OwnedFd::from_raw_fd(result) })
}

fn open_at(
    parent: &OwnedFd,
    name: &CStr,
    flags: libc::c_int,
    stage: &'static str,
) -> Result<OwnedFd, CgroupError> {
    // SAFETY: The parent descriptor and string are valid. The call omits `O_CREAT`.
    let result = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if result < 0 {
        return Err(errno(stage));
    }
    // SAFETY: openat returned a new descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(result) })
}

fn write_control(
    dir: &OwnedFd,
    name: &CStr,
    value: &str,
    stage: &'static str,
) -> Result<(), CgroupError> {
    let fd = open_at(dir, name, libc::O_WRONLY, stage)?;
    let bytes = value.as_bytes();
    // SAFETY: The buffer remains valid for the single write to the control file.
    let written = unsafe { libc::write(fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
    if written < 0 {
        return Err(errno(stage));
    }
    if written as usize != bytes.len() {
        return Err(CgroupError::ShortWrite { stage });
    }
    Ok(())
}

impl PreparedCgroup {
    /// Descriptors required by the assistant before joining the group.
    pub(in crate::process) fn helper_fds(&self) -> [libc::c_int; 2] {
        [self.parent.as_raw_fd(), self.directory.as_raw_fd()]
    }
    /// For interprocess order testing only; a conventional directory is not a cgroup.
    #[cfg(test)]
    pub(in crate::process) fn fixture(parent: OwnedFd, directory: OwnedFd) -> Self {
        Self {
            parent,
            directory,
            name: CString::new("fixture").unwrap(),
        }
    }
    /// Creates a separate group and sets all the limits until the result is returned.
    /// The root is passed on by a trusted compound; this challenge does not choose the path
    /// itself.
    pub(crate) fn create(
        root: BorrowedFd<'_>,
        name: &str,
        limits: Limits,
    ) -> Result<Self, CgroupError> {
        if name.is_empty()
            || name.len() > 96
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(CgroupError::InvalidName);
        }
        if !(1_000..=1_000_000).contains(&limits.cpu_period_us.get()) {
            return Err(CgroupError::InvalidCpuPeriod);
        }
        let name = CString::new(name).map_err(|_| CgroupError::InvalidName)?;
        let parent = duplicate(root)?;
        // SAFETY: statfs are initialized by the kernel upon successful call.
        let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: The descriptor and pointer are valid.
        if unsafe { libc::fstatfs(parent.as_raw_fd(), &mut stat) } < 0 {
            return Err(errno("inspect_root"));
        }
        if stat.f_type as libc::c_long != CGROUP2_SUPER_MAGIC {
            return Err(CgroupError::WrongFilesystem);
        }
        // SAFETY: name is one verified component; parent is open caller.
        if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } < 0 {
            return Err(errno("create"));
        }
        let directory = match open_at(
            &parent,
            &name,
            libc::O_RDONLY | libc::O_DIRECTORY,
            "open_child",
        ) {
            Ok(fd) => fd,
            Err(error) => {
                // SAFETY: We only delete the empty catalogue we have created.
                unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) };
                return Err(error);
            }
        };
        let group = Self {
            parent,
            directory,
            name,
        };
        write_control(
            &group.directory,
            c"cpu.max",
            &format!("{} {}", limits.cpu_quota_us, limits.cpu_period_us),
            "cpu.max",
        )?;
        write_control(
            &group.directory,
            c"memory.max",
            &limits.memory_bytes.to_string(),
            "memory.max",
        )?;
        write_control(
            &group.directory,
            c"pids.max",
            &limits.processes.to_string(),
            "pids.max",
        )?;
        Ok(group)
    }

    /// The assistant joins the ongoing process and waits for the parent's permission.
    /// Zero in cgroup.procs, the kernel resolves as an ongoing process: there is no reference
    /// to a numerical PID that could be reused. The attachment error closes the gate and
    /// prevents you from continuing execution.
    /// Restrictions and control of origin are carried out separately.
    pub(crate) fn join_current_and_wait(
        &self,
        receiver: crate::process::gate::GateReceiver,
        timeout: std::time::Duration,
    ) -> Result<(), CgroupError> {
        write_control(&self.directory, c"cgroup.procs", "0", "cgroup.procs")?;
        receiver.wait(timeout).map_err(CgroupError::Gate)
    }
}

impl Drop for PreparedCgroup {
    fn drop(&mut self) {
        // SAFETY: Only the name of the created group is deleted; the non-empty core will
        // retain it.
        unsafe {
            libc::unlinkat(
                self.parent.as_raw_fd(),
                self.name.as_ptr(),
                libc::AT_REMOVEDIR,
            )
        };
    }
}
