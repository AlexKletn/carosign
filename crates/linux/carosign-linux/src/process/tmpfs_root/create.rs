use super::RootError;
use super::RootStage;
use std::ffi::CString;
use std::io;
use std::num::NonZeroU64;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

// The numbers are only for the ABI Linux mount API.
const FSOPEN_CLOEXEC: u32 = 1;

const FSCONFIG_SET_STRING: u32 = 1;

const FSCONFIG_CMD_CREATE: u32 = 6;

const FSMOUNT_CLOEXEC: u32 = 1;

const MOUNT_ATTR_NOSUID: u32 = 2;

const MOUNT_ATTR_NODEV: u32 = 4;

pub(super) fn error(stage: RootStage) -> RootError {
    RootError {
        stage,
        source: io::Error::last_os_error(),
    }
}

/// Creates a limited tmpfs without attaching it to the mounting tree.
/// This prevents the worker from getting the old one. `/dev` through this facility.
pub(crate) fn create(size_bytes: NonZeroU64) -> Result<OwnedFd, RootError> {
    // SAFETY: Static string and kernel-defined fsopen arguments.
    let raw = unsafe { libc::syscall(libc::SYS_fsopen, c"tmpfs".as_ptr(), FSOPEN_CLOEXEC) };
    if raw < 0 {
        return Err(error(RootStage::OpenFilesystem));
    }
    // SAFETY: A successful fsopen returns a new owned descriptor.
    let context = unsafe { OwnedFd::from_raw_fd(raw as i32) };
    let size = CString::new(size_bytes.to_string()).expect("число не содержит NUL");
    for (key, value, stage) in [
        (c"size", size.as_c_str(), RootStage::SetSize),
        (c"mode", c"0700", RootStage::SetMode),
    ] {
        // SAFETY: The strings and descriptor are valid until the kernel returns.
        if unsafe {
            libc::syscall(
                libc::SYS_fsconfig,
                context.as_raw_fd(),
                FSCONFIG_SET_STRING,
                key.as_ptr(),
                value.as_ptr(),
                0,
            )
        } < 0
        {
            return Err(error(stage));
        }
    }
    // SAFETY: The creation command does not use key/value pointers.
    if unsafe {
        libc::syscall(
            libc::SYS_fsconfig,
            context.as_raw_fd(),
            FSCONFIG_CMD_CREATE,
            std::ptr::null::<libc::c_char>(),
            std::ptr::null::<libc::c_char>(),
            0,
        )
    } < 0
    {
        return Err(error(RootStage::CreateFilesystem));
    }
    // SAFETY: Context is the ready-made context of the file system.
    let mount = unsafe {
        libc::syscall(
            libc::SYS_fsmount,
            context.as_raw_fd(),
            FSMOUNT_CLOEXEC,
            MOUNT_ATTR_NOSUID | MOUNT_ATTR_NODEV,
        )
    };
    if mount < 0 {
        return Err(error(RootStage::CreateMount));
    }
    // SAFETY: A successful fsmount returns a new mounting descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(mount as i32) })
}
