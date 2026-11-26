//! Execution of a fixed ELF object within a separate assistant.
//!
//! This mechanism does not confirm origin, immutability or isolation.
//! It is permissible to call it only after other mandatory checks.

#![allow(dead_code)]

use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};

const MAX_ARGS: usize = 32;
const MAX_ARG_BYTES: usize = 4096;

/// Runs a previously open-to-read ELF file without allowing the path to be re-opened.
/// If it's successful, it doesn't come back. FD_CLOEXECExecutable ELF format does not require
/// the descriptor to be transmitted to the interpreter.
/// Scripts and unread objects are deflected to a system call.
pub(crate) fn execute(executable: BorrowedFd<'_>, args: &[CString]) -> io::Result<()> {
    if args.is_empty()
        || args.len() > MAX_ARGS
        || args.iter().map(|v| v.as_bytes().len()).sum::<usize>() > MAX_ARG_BYTES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid arguments",
        ));
    }
    // SAFETY: The structure and descriptor are valid during the call.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(executable.as_raw_fd(), &mut stat) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    let mut magic = [0_u8; 4];
    // SAFETY: The buffer is available for four bytes, the file position does not change.
    let count = unsafe {
        libc::pread(
            executable.as_raw_fd(),
            magic.as_mut_ptr().cast(),
            magic.len(),
            0,
        )
    };
    if count < 0 {
        return Err(io::Error::last_os_error());
    }
    if count != 4 || magic != *b"\x7fELF" {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "ELF required"));
    }
    let mut argv: Vec<*const libc::c_char> = args.iter().map(|arg| arg.as_ptr()).collect();
    argv.push(std::ptr::null());
    let path: &CStr = c"PATH=/usr/bin:/bin";
    let env = [path.as_ptr(), std::ptr::null()];
    // SAFETY: The argv/env pointers end in null and are valid until return; the empty path
    // addresses the open descriptor rather than being allowed by name.
    unsafe {
        libc::syscall(
            libc::SYS_execveat,
            executable.as_raw_fd(),
            c"".as_ptr(),
            argv.as_ptr(),
            env.as_ptr(),
            libc::AT_EMPTY_PATH,
        );
    }
    // Do not repeat the challenge: the error returned, including EINTR, is step failure.
    Err(io::Error::last_os_error())
}

#[path = "pinned_exec.tests.rs"]
#[cfg(test)]
mod tests;
