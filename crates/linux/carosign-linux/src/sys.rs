//! Linux syscall wrappers.
//!
//! These wrappers validate inputs and preserve errors returned by the kernel.

#![allow(unsafe_code)]

use core::ffi::{CStr, c_int, c_void};
use std::ffi::CString;
use std::fs::File;
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd, RawFd};

mod file_identity;
mod launch_metadata;
mod peer_user;
mod socket_pair;
mod sys_error;

pub use file_identity::FileIdentity;
pub(crate) use launch_metadata::LaunchMetadata;
pub use peer_user::PeerUser;
pub use socket_pair::SocketPair;
pub use sys_error::SysError;

fn errno() -> i32 {
    std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(libc::EIO)
}

// -
// --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

/// Monoton boot time, ns.
///
/// ⚠️ `CLOCK_BOOTTIME` It includes sleep, which is the only clock that would be suitable for
/// deadlines, but the download number changes, and the deadline without it is someone else's
/// time (document 05).
pub fn boottime_ns() -> Result<u64, SysError> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: The pointer refers to the owned `timespec`.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    if rc != 0 {
        return Err(SysError { errno: errno() });
    }
    timespec_ns(ts)
}

fn timespec_ns(ts: libc::timespec) -> Result<u64, SysError> {
    let invalid = || SysError {
        errno: libc::EINVAL,
    };
    let seconds = u64::try_from(ts.tv_sec).map_err(|_| invalid())?;
    let nanos = u64::try_from(ts.tv_nsec).map_err(|_| invalid())?;
    if nanos >= 1_000_000_000 {
        return Err(invalid());
    }
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(nanos))
        .ok_or(SysError {
            errno: libc::EOVERFLOW,
        })
}

/// Current download number.
///
/// ⚠️ Reading from `boot_id` kernels, not made-up.
pub fn boot_id() -> Result<[u8; 16], SysError> {
    let text =
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id").map_err(|error| SysError {
            errno: error.raw_os_error().unwrap_or(libc::EIO),
        })?;
    parse_boot_id(&text)
}

fn parse_boot_id(text: &str) -> Result<[u8; 16], SysError> {
    let text = text.trim();
    let bytes = text.as_bytes();
    if bytes.len() != 36 || [8, 13, 18, 23].iter().any(|index| bytes[*index] != b'-') {
        return Err(SysError {
            errno: libc::EINVAL,
        });
    }
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return Err(SysError {
            errno: libc::EINVAL,
        });
    }
    let mut out = [0u8; 16];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let text = core::str::from_utf8(chunk).map_err(|_| SysError {
            errno: libc::EINVAL,
        })?;
        out[index] = u8::from_str_radix(text, 16).map_err(|_| SysError {
            errno: libc::EINVAL,
        })?;
    }
    if out.iter().all(|byte| *byte == 0) {
        return Err(SysError {
            errno: libc::EINVAL,
        });
    }
    Ok(out)
}

// -
// --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

/// Opens the file for reading only, with `O_CLOEXEC`.
pub fn open_read(path: &str) -> Result<OwnedFd, SysError> {
    let c_path = CString::new(path).map_err(|_| SysError {
        errno: libc::EINVAL,
    })?;
    // SAFETY: The string is NUL-terminated, and the return value is checked.
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: `fd` It is non-negative and no longer belongs to us separately.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Fixes the object without opening it for reading and without clicking on links.
/// The descriptor and its metadata alone do not certify the package or policy.
pub(crate) fn open_launch_candidate(path: &str) -> Result<OwnedFd, SysError> {
    let c_path = CString::new(path).map_err(|_| SysError {
        errno: libc::EINVAL,
    })?;
    open_launch_at(libc::AT_FDCWD, &c_path, false)
}

/// Opens one component of the already fixed directory.
pub(crate) fn open_launch_component(
    parent: &OwnedFd,
    name: &std::ffi::OsStr,
) -> Result<OwnedFd, SysError> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes == b"." || bytes == b".." || bytes.contains(&b'/') {
        return Err(SysError {
            errno: libc::EINVAL,
        });
    }
    let path = CString::new(bytes).map_err(|_| SysError {
        errno: libc::EINVAL,
    })?;
    open_launch_at(parent.as_raw_fd(), &path, true)
}

fn open_launch_at(parent: RawFd, path: &CStr, beneath: bool) -> Result<OwnedFd, SysError> {
    // SAFETY: zero-value open_how are acceptable; below are the required fields.
    let mut how: libc::open_how = unsafe { core::mem::zeroed() };
    how.flags = (libc::O_PATH | libc::O_CLOEXEC) as u64;
    how.resolve = libc::RESOLVE_NO_SYMLINKS | libc::RESOLVE_NO_MAGICLINKS;
    if beneath {
        how.resolve |= libc::RESOLVE_BENEATH;
    }
    // SAFETY: The path and structure are valid until the return; the kernel copies them.
    // ENOSYS and other errors are returned to the caller without bypassing.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            parent,
            path.as_ptr(),
            &how as *const libc::open_how,
            core::mem::size_of::<libc::open_how>(),
        )
    };
    if fd < 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: The successful call gave us a new descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as c_int) })
}

pub(crate) fn launch_metadata(fd: &OwnedFd) -> Result<LaunchMetadata, SysError> {
    // SAFETY: The structure and descriptor are valid at fstat.
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if unsafe { libc::fstat(fd.as_raw_fd(), &mut st) } != 0 {
        return Err(SysError { errno: errno() });
    }
    Ok(LaunchMetadata {
        identity: file_identity(&st),
        mode: st.st_mode,
    })
}

/// Reopens an already fixed normal file to read through procfs.
/// The name of the packet is not allowed again; the caller is obliged to check the metadata
/// of both descriptors, the origin and immutability of the file is not proved by this.
pub(crate) fn reopen_pinned_regular(fd: &OwnedFd) -> Result<OwnedFd, SysError> {
    let metadata = launch_metadata(fd)?;
    if metadata.mode & libc::S_IFMT != libc::S_IFREG {
        return Err(SysError {
            errno: libc::EINVAL,
        });
    }
    // SAFETY: The string is static and NUL-terminated.
    let proc_fd = unsafe {
        libc::open(
            c"/proc/self/fd".as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if proc_fd < 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: Successful open gave us the descriptor.
    let proc_fd = unsafe { OwnedFd::from_raw_fd(proc_fd) };
    // SAFETY: The kernel fills the correct structure with successful fstatfs.
    let mut fs: libc::statfs = unsafe { core::mem::zeroed() };
    if unsafe { libc::fstatfs(proc_fd.as_raw_fd(), &mut fs) } != 0 {
        return Err(SysError { errno: errno() });
    }
    const PROC_SUPER_MAGIC: libc::c_long = 0x9fa0;
    if fs.f_type as libc::c_long != PROC_SUPER_MAGIC {
        return Err(SysError {
            errno: libc::EINVAL,
        });
    }
    let name = CString::new(fd.as_raw_fd().to_string()).expect("числовой дескриптор");
    // Follow only the reference of the fixed descriptor in this procfs.
    // O_NONBLOCK and O_NOCTTY In addition, the wait and control of tty are excluded.
    // SAFETY: The directory descriptor and string remain valid throughout the call.
    let reopened = unsafe {
        libc::openat(
            proc_fd.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_NOCTTY,
        )
    };
    if reopened < 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: A successful openat handed over a new descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(reopened) })
}

/// Opens the device for reading and writing, with `O_CLOEXEC`.
pub fn open_device(path: &str) -> Result<OwnedFd, SysError> {
    let c_path = CString::new(path).map_err(|_| SysError {
        errno: libc::EINVAL,
    })?;
    // SAFETY: The string is NUL-terminated, and the return value is checked.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOCTTY,
        )
    };
    if fd < 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: `fd` It is non-negative and comes to us.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// What does the kernel say about the file?
pub fn stat(path: &str) -> Result<FileIdentity, SysError> {
    let c_path = CString::new(path).map_err(|_| SysError {
        errno: libc::EINVAL,
    })?;
    // SAFETY: The kernel initializes the structure.
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    // SAFETY: The path is valid, pointing to our structure.
    let rc = unsafe { libc::lstat(c_path.as_ptr(), &mut st) };
    if rc != 0 {
        return Err(SysError { errno: errno() });
    }
    Ok(file_identity(&st))
}

fn file_identity(st: &libc::stat) -> FileIdentity {
    FileIdentity {
        regular: (st.st_mode & libc::S_IFMT) == libc::S_IFREG,
        uid: st.st_uid,
        world_writable: (st.st_mode & 0o002) != 0,
        device: st.st_dev,
        inode: st.st_ino,
    }
}

// -
// --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

/// Process description for observation.
#[derive(Debug)]
pub struct PidFd {
    fd: OwnedFd,
}

impl PidFd {
    /// Returns an independent copy of the retained process descriptor.
    pub(crate) fn duplicate(&self) -> Result<OwnedFd, SysError> {
        self.fd.try_clone().map_err(|error| SysError {
            errno: error.raw_os_error().unwrap_or(libc::EIO),
        })
    }
    /// It gets a stable link to the peer connected socket.
    /// This does not confirm either the establishment or the authority of the process.
    pub fn from_socket_peer(socket: &OwnedFd) -> Result<Self, SysError> {
        // Meaning. SO_PEERPIDFD Linux UAPI asm-generic/socket.h.
        const SO_PEERPIDFD: c_int = 77;
        let mut raw: c_int = -1;
        let mut length = core::mem::size_of::<c_int>() as libc::socklen_t;
        // SAFETY: The kernel writes a single descriptor into the initialized memory.
        let result = unsafe {
            libc::getsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                SO_PEERPIDFD,
                (&mut raw as *mut c_int).cast::<c_void>(),
                &mut length,
            )
        };
        if result != 0 {
            return Err(SysError { errno: errno() });
        }
        if raw < 0 {
            return Err(SysError {
                errno: libc::EPROTO,
            });
        }
        // SAFETY: The successful `getsockopt` transferred ownership of the new descriptor.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        if length as usize != core::mem::size_of::<c_int>() {
            return Err(SysError {
                errno: libc::EPROTO,
            });
        }
        // Prevent inheritance if kernel behavior differs from the expected behavior.
        // SAFETY: F_GETFD It does not change the memory and accepts the current fd.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
        if flags < 0 {
            return Err(SysError { errno: errno() });
        }
        if flags & libc::FD_CLOEXEC == 0 {
            return Err(SysError {
                errno: libc::EPROTO,
            });
        }
        Ok(Self { fd })
    }

    /// Opens. `pidfd`.
    ///
    /// ⚠️ `pidfd` preferably `kill(pid, 0)`The process number is reused, but the descriptor
    /// is not.
    pub fn open(pid: u32) -> Result<Self, SysError> {
        if pid == 0 || pid > libc::pid_t::MAX as u32 {
            return Err(SysError {
                errno: libc::EINVAL,
            });
        }
        // SAFETY: `pidfd_open` with proven arguments.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0) };
        if fd < 0 {
            return Err(SysError { errno: errno() });
        }
        // SAFETY: `fd` is nonnegative.
        Ok(Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd as RawFd) },
        })
    }

    /// Whether the process is dead.
    ///
    /// ⚠️ “Living” is only “living”: it says nothing about hardware.
    pub fn has_exited(&self) -> Result<bool, SysError> {
        // SAFETY: poll with one element; the structure is filled with the core.
        let mut pollfd = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: Timeout 0 is not waiting for us.
        let rc = unsafe { libc::poll(&mut pollfd, 1, 0) };
        if rc < 0 {
            return Err(SysError { errno: errno() });
        }
        if rc == 0 {
            return Ok(false);
        }
        if pollfd.revents & libc::POLLNVAL != 0 {
            return Err(SysError { errno: libc::EBADF });
        }
        // Pidfd. `POLLHUP` This means that the process has already been assembled by wait(2).
        // This is also a conclusion, not a proof that the worker is alive.
        if pollfd.revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            return Ok(true);
        }
        // An unknown event is not a sign of a living worker.
        Err(SysError { errno: libc::EIO })
    }

    /// It's the process that's been held back, not the reusable numerical PID, and the rest
    /// of the device is not confirmed by that.
    #[allow(dead_code)]
    pub fn kill(&self) -> Result<(), SysError> {
        // SAFETY: pidfd is retained OwnedFdThe rest of the arguments are given to the core.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.fd.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if result != 0 {
            return Err(SysError { errno: errno() });
        }
        Ok(())
    }

    /// Expects only a sign of exit, not replacing the observation of the collection of the
    /// child.
    /// At the end of the term returns false, if the error - refusal.
    #[allow(dead_code)]
    pub fn wait_exited(&self, timeout: std::time::Duration) -> Result<bool, SysError> {
        let start = std::time::Instant::now();
        loop {
            let remaining = timeout.saturating_sub(start.elapsed());
            // Rounding up does not turn a non-zero term into an immediate poll.
            let ms = remaining.as_millis().saturating_add(u128::from(
                !remaining.subsec_nanos().is_multiple_of(1_000_000),
            ));
            let millis = ms.min(i32::MAX as u128) as i32;
            let mut event = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: The poll receives a valid record and a limited period of time.
            let result = unsafe { libc::poll(&mut event, 1, millis) };
            if result < 0 {
                let error = errno();
                if error == libc::EINTR && start.elapsed() < timeout {
                    continue;
                }
                return Err(SysError { errno: error });
            }
            if result == 0 {
                if start.elapsed() < timeout {
                    continue;
                }
                return Ok(false);
            }
            if event.revents & libc::POLLNVAL != 0 {
                return Err(SysError { errno: libc::EBADF });
            }
            if event.revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                return Ok(true);
            }
            return Err(SysError { errno: libc::EIO });
        }
    }
}

use std::os::fd::AsRawFd;

// -
// --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

/// Makes a couple of sockets. `AF_UNIX/SOCK_SEQPACKET` s `SOCK_CLOEXEC`.
pub fn socketpair_seqpacket() -> Result<SocketPair, SysError> {
    let mut fds = [0 as c_int; 2];
    // SAFETY: The array has space for the two entries the kernel writes.
    let rc = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    if rc != 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: The kernel initialized both entries.
    Ok(unsafe {
        SocketPair {
            platform: OwnedFd::from_raw_fd(fds[0]),
            worker: OwnedFd::from_raw_fd(fds[1]),
        }
    })
}

/// Event counter.
pub fn eventfd() -> Result<OwnedFd, SysError> {
    // SAFETY: A normal call with non-zero flags.
    let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
    if fd < 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: `fd` is nonnegative.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// File in memory with `CLOEXEC`.
///
/// ⚠️ The descriptor is open to read and write. The seals are superimposed separately; the
/// broker's contract requires a separate descriptor that is read-only.
pub fn memfd(name: &str) -> Result<OwnedFd, SysError> {
    let c_name = CString::new(name).map_err(|_| SysError {
        errno: libc::EINVAL,
    })?;
    // SAFETY: The name ended in zero.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_memfd_create,
            c_name.as_ptr(),
            // ⚠️ `MFD_ALLOW_SEALING` It is mandatory: without it, seals are not put.
            (libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) as libc::c_long,
        )
    };
    if fd < 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: `fd` is nonnegative.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as RawFd) })
}

/// Seal the file in memory: read only, without changing the size.
///
/// ⚠️ Printing forbids changing the contents and size, but does not change the mode of
/// opening the descriptor itself. `F_SEAL_WRITE` requires the absence of shared displays that
/// allow recording.
pub fn seal_read_only(fd: &OwnedFd) -> Result<(), SysError> {
    let required = libc::F_SEAL_WRITE | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_SEAL;
    // SAFETY: fd valid, length 0 – print the entire file.
    let rc = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_ADD_SEALS, required) };
    if rc != 0 {
        return Err(SysError { errno: errno() });
    }
    Ok(())
}

/// Checks the full set of seals and read-only opening mode.
///
/// These checks themselves are not enough for admission: the recipient additionally checks
/// the file type and size, content, and transmission authority.
pub fn is_sealed(fd: &OwnedFd) -> Result<bool, SysError> {
    // SAFETY: fd is valid, F_GETFL It reads the opening mode.
    let mode = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if mode < 0 {
        return Err(SysError { errno: errno() });
    }
    if mode & libc::O_ACCMODE != libc::O_RDONLY || mode & libc::O_PATH != 0 {
        return Ok(false);
    }
    // SAFETY: It's just a call.
    let rc = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GET_SEALS) };
    if rc < 0 {
        return Err(SysError { errno: errno() });
    }
    let required = libc::F_SEAL_WRITE | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_SEAL;
    Ok((rc & required) == required)
}

/// Transmits the descriptor over the socket.`SCM_RIGHTS`) with one byte tag.
///
/// ⚠️ One descriptor, one message. The packet transmission of slots in one frame is made a
/// separate function when the real one appears. `Bind`.
pub fn send_fd(socket: &OwnedFd, fd: &OwnedFd) -> Result<(), SysError> {
    send_fds(socket, &[fd])
}

/// Transmits several descriptors in a single message.
pub fn send_fds(socket: &OwnedFd, fds: &[&OwnedFd]) -> Result<(), SysError> {
    if fds.is_empty() || fds.len() > 8 {
        return Err(SysError {
            errno: libc::EINVAL,
        });
    }
    let mut byte = [0u8; 1];
    let mut iov = libc::iovec {
        iov_base: byte.as_mut_ptr() as *mut c_void,
        iov_len: 1,
    };
    let space =
        unsafe { libc::CMSG_SPACE((fds.len() * core::mem::size_of::<c_int>()) as u32) } as usize;
    let mut control = vec![0usize; space.div_ceil(core::mem::size_of::<usize>())];
    let mut msg: libc::msghdr = unsafe { core::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr() as *mut c_void;
    msg.msg_controllen = space as _;

    // SAFETY: The control header is empty and contains all the descriptors.
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        if cmsg.is_null() {
            return Err(SysError {
                errno: libc::EINVAL,
            });
        }
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN((fds.len() * core::mem::size_of::<c_int>()) as u32) as _;
        let data = libc::CMSG_DATA(cmsg) as *mut c_int;
        for (index, fd) in fds.iter().enumerate() {
            *data.add(index) = fd.as_raw_fd();
        }
    }

    // SAFETY: The message is complete, sender is ours.
    let rc = unsafe {
        libc::sendmsg(
            socket.as_raw_fd(),
            &msg,
            libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT,
        )
    };
    if rc < 0 {
        return Err(SysError { errno: errno() });
    }
    if rc != 1 {
        return Err(SysError { errno: libc::EIO });
    }
    Ok(())
}

/// He's taking descriptors from the socket.
///
/// ⚠️ Returns exactly what came. The caller makes the reconciliation with the announced
/// slots; the extra ** should be closed, not left as is.
pub fn recv_fds(socket: &OwnedFd, max_fds: usize) -> Result<Vec<OwnedFd>, SysError> {
    if max_fds == 0 || max_fds > 8 {
        return Err(SysError {
            errno: libc::EINVAL,
        });
    }
    let mut byte = [0u8; 1];
    let mut iov = libc::iovec {
        iov_base: byte.as_mut_ptr() as *mut c_void,
        iov_len: 1,
    };
    let space =
        unsafe { libc::CMSG_SPACE((max_fds * core::mem::size_of::<c_int>()) as u32) } as usize;
    let mut control = vec![0usize; space.div_ceil(core::mem::size_of::<usize>())];
    let mut msg: libc::msghdr = unsafe { core::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr() as *mut c_void;
    msg.msg_controllen = space as _;

    // SAFETY: The buffer is ours, the size is known.
    let rc = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut msg, libc::MSG_CMSG_CLOEXEC) };
    if rc < 0 {
        return Err(SysError { errno: errno() });
    }
    let mut out = Vec::new();
    // SAFETY: The control header is filled with the kernel, the length is checked.
    unsafe {
        let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
        while !cmsg.is_null() {
            if (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
                let count =
                    ((*cmsg).cmsg_len - libc::CMSG_LEN(0) as usize) / core::mem::size_of::<c_int>();
                let data = libc::CMSG_DATA(cmsg) as *const c_int;
                for index in 0..count {
                    let fd = *data.add(index);
                    if fd >= 0 {
                        out.push(OwnedFd::from_raw_fd(fd));
                    }
                }
            }
            cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
        }
    }
    if msg.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC) != 0 || out.len() > max_fds {
        // The resulting descriptors already belong to `out`When they come out, they close,
        // including those that the kernel has managed to install before pruning.
        return Err(SysError {
            errno: libc::EMSGSIZE,
        });
    }
    if rc != 1 || byte[0] != 0 {
        return Err(SysError {
            errno: libc::EBADMSG,
        });
    }
    Ok(out)
}

/// Accepting the connection.
///
/// ⚠️ Listener-- `AF_UNIX`-socket, created in advance; here only `accept4` s `SOCK_CLOEXEC`so
/// that the received descriptor does not survive `exec`.
pub fn accept_connection(listener: &OwnedFd) -> Result<(OwnedFd, PeerUser), SysError> {
    // SAFETY: The listener is valid and belongs to us.
    let fd = unsafe {
        libc::accept4(
            listener.as_raw_fd(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            libc::SOCK_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: `fd` is nonnegative.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let user = peer_credentials(&fd)?;
    Ok((fd, user))
}

/// Takes it off. `SO_PEERCRED` from the connection.
pub fn peer_credentials(socket: &OwnedFd) -> Result<PeerUser, SysError> {
    // SAFETY: The kernel fills the `ucred` structure.
    let mut credentials: libc::ucred = unsafe { core::mem::zeroed() };
    let mut length = core::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: The index and the length of our structure.
    let rc = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut credentials as *mut libc::ucred as *mut c_void,
            &mut length,
        )
    };
    if rc != 0 {
        return Err(SysError { errno: errno() });
    }
    checked_peer_credentials(credentials, length)
}

// An incomplete response or PID that is not represented by a positive process identifier in
// this namespace does not identify the customer.
fn checked_peer_credentials(
    credentials: libc::ucred,
    length: libc::socklen_t,
) -> Result<PeerUser, SysError> {
    if length as usize != core::mem::size_of::<libc::ucred>() || credentials.pid <= 0 {
        return Err(SysError {
            errno: libc::EPROTO,
        });
    }
    Ok(PeerUser {
        pid: credentials.pid as u32,
        uid: credentials.uid,
        gid: credentials.gid,
    })
}

/// Creates a listener along the way.
///
/// ⚠️ If the path is busy - refusal, not deletion of someone else: only the one who created
/// it has the right to delete the socket file (otherwise you can intercept the service).
pub fn listen_unix(path: &str) -> Result<OwnedFd, SysError> {
    let c_path = CString::new(path).map_err(|_| SysError {
        errno: libc::EINVAL,
    })?;
    // SAFETY: It's just a call.
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: `fd` is nonnegative.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };

    let mut address: libc::sockaddr_un = unsafe { core::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let bytes = path.as_bytes();
    if bytes.len() >= address.sun_path.len() {
        return Err(SysError {
            errno: libc::ENAMETOOLONG,
        });
    }
    for (index, byte) in bytes.iter().enumerate() {
        address.sun_path[index] = *byte as libc::c_char;
    }
    let length = (core::mem::size_of::<libc::sa_family_t>() + bytes.len() + 1) as libc::socklen_t;
    // SAFETY: The address is full, the length is correct.
    let rc = unsafe {
        libc::bind(
            fd.as_raw_fd(),
            &address as *const libc::sockaddr_un as *const libc::sockaddr,
            length,
        )
    };
    if rc != 0 {
        return Err(SysError { errno: errno() });
    }
    // SAFETY: It's just a call.
    let rc = unsafe { libc::listen(fd.as_raw_fd(), 8) };
    if rc != 0 {
        return Err(SysError { errno: errno() });
    }
    let _ = c_path;
    Ok(fd)
}

/// Sixteen random bytes at the core.
///
/// An unready source returns an error, rather than blocking the flow or giving out a
/// predictable value.
pub fn random_16() -> Result<[u8; 16], SysError> {
    fill_random(|remaining| {
        // SAFETY: The buffer belongs to the caller and is valid at the time of the call.
        let rc = unsafe {
            libc::getrandom(
                remaining.as_mut_ptr().cast::<c_void>(),
                remaining.len(),
                libc::GRND_NONBLOCK,
            )
        };
        if rc < 0 {
            Err(SysError { errno: errno() })
        } else {
            Ok(rc as usize)
        }
    })
}

// Interruptions and short readings are permissible, but you can’t wait for the kernel
// indefinitely.
fn fill_random(
    mut read: impl FnMut(&mut [u8]) -> Result<usize, SysError>,
) -> Result<[u8; 16], SysError> {
    const INTERRUPT_LIMIT: u8 = 8;
    let mut bytes = [0u8; 16];
    let mut filled = 0;
    let mut interrupts = 0;
    while filled < bytes.len() {
        match read(&mut bytes[filled..]) {
            Ok(count) if count > 0 && count <= bytes.len() - filled => filled += count,
            Ok(_) => return Err(SysError { errno: libc::EIO }),
            Err(error) if error.errno == libc::EINTR && interrupts < INTERRUPT_LIMIT => {
                interrupts += 1;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(bytes)
}

/// Is there an accident right now?
pub fn random_available() -> bool {
    random_16().is_ok()
}

/// Closes the descriptor without checks (for refusal with excess).
pub fn close_all(fds: Vec<OwnedFd>) {
    drop(fds);
}

/// Transfer to `File` (Reading in the same way as everywhere else.)
pub fn into_file(fd: OwnedFd) -> File {
    // SAFETY: Ownership transfers to `File`.
    unsafe { File::from_raw_fd(fd.into_raw_fd()) }
}

/// Checks the string as a C-line (without zero inside).
pub fn c_str(value: &str) -> bool {
    CStr::from_bytes_with_nul(value.as_bytes()).is_ok()
}

#[path = "sys.tests.rs"]
#[cfg(test)]
mod tests;

/// Connects a private local packet socket without waiting for a free queue.
pub fn connect_seqpacket(path: &std::path::Path) -> std::io::Result<OwnedFd> {
    use std::os::unix::ffi::OsStrExt;
    let name = path.as_os_str().as_bytes();
    // SAFETY: zero-point sockaddr_un Let's take it until it's full.
    let mut address: libc::sockaddr_un = unsafe { core::mem::zeroed() };
    if !path.is_absolute() || name.contains(&0) || name.len() >= address.sun_path.len() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (dst, src) in address.sun_path.iter_mut().zip(name) {
        *dst = *src as libc::c_char;
    }
    // SAFETY: A successful socket returns a new descriptor.
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    if raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: Ownership of the new descriptor is taken exactly once.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let len = core::mem::offset_of!(libc::sockaddr_un, sun_path) + name.len() + 1;
    // SAFETY: The completed address is valid at the time of the connect.
    if unsafe {
        libc::connect(
            raw,
            (&address as *const libc::sockaddr_un).cast(),
            len as libc::socklen_t,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(fd)
}

/// It is exchanged in one package without auxiliary descriptors and with a limited period.
pub fn exchange_seqpacket(
    fd: &OwnedFd,
    request: &[u8],
    buffer: &mut [u8],
    timeout: std::time::Duration,
) -> std::io::Result<usize> {
    send_seqpacket(fd, request)?;
    receive_seqpacket(fd, buffer, timeout)
}

fn wait_readable(fd: &OwnedFd, timeout: std::time::Duration) -> std::io::Result<()> {
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let millis = timeout.as_millis().min(i32::MAX as u128) as i32;
    // SAFETY: The pointer passed to `poll` refers to one valid element.
    let ready = unsafe { libc::poll(&mut poll, 1, millis) };
    if ready < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if ready == 0 {
        return Err(std::io::ErrorKind::TimedOut.into());
    }
    Ok(())
}

pub fn receive_fd_until(
    socket: &OwnedFd,
    timeout: std::time::Duration,
) -> std::io::Result<OwnedFd> {
    wait_readable(socket, timeout)?;
    recv_fds(socket, 1)
        .map_err(|error| std::io::Error::from_raw_os_error(error.errno))?
        .pop()
        .ok_or_else(|| std::io::ErrorKind::InvalidData.into())
}

pub fn send_seqpacket(fd: &OwnedFd, packet: &[u8]) -> std::io::Result<()> {
    // SAFETY: fd and packet remain valid throughout send.
    let sent = unsafe {
        libc::send(
            fd.as_raw_fd(),
            packet.as_ptr().cast(),
            packet.len(),
            libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT,
        )
    };
    if sent < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if sent as usize != packet.len() {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(())
}

/// Accepts exactly one packet without auxiliary descriptors before the expiration date.
pub fn receive_seqpacket(
    fd: &OwnedFd,
    buffer: &mut [u8],
    timeout: std::time::Duration,
) -> std::io::Result<usize> {
    wait_readable(fd, timeout)?;
    let mut iov = libc::iovec {
        iov_base: buffer.as_mut_ptr().cast(),
        iov_len: buffer.len(),
    };
    // SAFETY: For example, if you have a zero msghdr, iov lives until recvmsg returns.
    let mut message: libc::msghdr = unsafe { core::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    // SAFETY: The buffer and header are available to the kernel at recvmsg time.
    let size = unsafe {
        libc::recvmsg(
            fd.as_raw_fd(),
            &mut message,
            libc::MSG_CMSG_CLOEXEC | libc::MSG_DONTWAIT,
        )
    };
    if size < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if size == 0 || message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(size as usize)
}

/// Checks the family and packet type of trusted input descriptor.
pub fn require_unix_seqpacket(fd: &OwnedFd) -> std::io::Result<()> {
    for (option, expected) in [
        (libc::SO_DOMAIN, libc::AF_UNIX),
        (libc::SO_TYPE, libc::SOCK_SEQPACKET),
    ] {
        let mut value: c_int = 0;
        let mut len = core::mem::size_of_val(&value) as libc::socklen_t;
        // SAFETY: The output value and length are available to the kernel during getockopt.
        if unsafe {
            libc::getsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                option,
                (&mut value as *mut c_int).cast(),
                &mut len,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        if len as usize != core::mem::size_of_val(&value) || value != expected {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
    }
    Ok(())
}

/// Accepts a connection with limited waiting. The listener belongs to one receiver.
pub fn accept_seqpacket_until(
    listener: &OwnedFd,
    timeout: std::time::Duration,
) -> std::io::Result<(OwnedFd, PeerUser)> {
    // SAFETY: F_GETFL Reads the current flags of the open descriptor.
    let flags = unsafe { libc::fcntl(listener.as_raw_fd(), libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: listener valid. O_NONBLOCK Applies only to the listener; accept4
    // accept_connection It creates a separate blocking descriptor.
    if unsafe {
        libc::fcntl(
            listener.as_raw_fd(),
            libc::F_SETFL,
            flags | libc::O_NONBLOCK,
        )
    } < 0
    {
        return Err(std::io::Error::last_os_error());
    }
    wait_readable(listener, timeout)?;
    accept_connection(listener).map_err(|error| std::io::Error::from_raw_os_error(error.errno))
}
