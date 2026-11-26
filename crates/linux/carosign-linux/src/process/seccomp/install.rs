use super::FilterError;
use std::io;

// The classic BPF and seccomp codes only apply to the Linux kernel.
const LOAD_WORD_ABS: u16 = 0x20;
const JUMP_EQUAL: u16 = 0x15;
const RETURN: u16 = 0x06;
const KILL_PROCESS: u32 = 0x8000_0000;
const ALLOW: u32 = 0x7fff_0000;
const ERRNO: u32 = 0x0005_0000;
pub(super) const MAX_CALLS: usize = 128;

#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xc000_003e;
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xc000_00b7;

fn instruction(code: u16, jt: u8, jf: u8, k: u32) -> libc::sock_filter {
    libc::sock_filter { code, jt, jf, k }
}

/// Set allowlist in the current single-threaded process. Failure or partial setup requires
/// the assistant to be terminated. The number of calls is known only to the Linux adapter;
/// they do not cross the domain layer contract.
pub(crate) fn install(allowed: &[libc::c_long]) -> Result<(), FilterError> {
    if allowed.is_empty()
        || allowed.len() > MAX_CALLS
        || allowed.iter().any(|&n| !(0..0x4000_0000).contains(&n))
        // ADR supervisor bans new sockets in the base profile. io_uring You can create a
        // socket bypassing a direct system call.
        || allowed.iter().any(|n| matches!(*n,
            libc::SYS_socket | libc::SYS_socketpair | libc::SYS_io_uring_setup
                | libc::SYS_io_uring_enter | libc::SYS_io_uring_register))
    {
        return Err(FilterError::InvalidPolicy);
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        let _ = allowed;
        return Err(FilterError::UnsupportedArchitecture);
    }
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        // SAFETY: Reading an irreversible attribute of the current process without pointers.
        let nnp = unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) };
        if nnp < 0 {
            return Err(FilterError::Kernel(io::Error::last_os_error()));
        }
        if nnp != 1 {
            return Err(FilterError::NoNewPrivilegesRequired);
        }
        let mut filter = Vec::with_capacity(5 + allowed.len() * 2);
        // seccomp_data.arch has a displacement of 4, nr - 0.
        filter.push(instruction(LOAD_WORD_ABS, 0, 0, 4));
        filter.push(instruction(JUMP_EQUAL, 1, 0, AUDIT_ARCH));
        filter.push(instruction(RETURN, 0, 0, KILL_PROCESS));
        filter.push(instruction(LOAD_WORD_ABS, 0, 0, 0));
        for &number in allowed {
            filter.push(instruction(JUMP_EQUAL, 0, 1, number as u32));
            filter.push(instruction(RETURN, 0, 0, ALLOW));
        }
        filter.push(instruction(RETURN, 0, 0, ERRNO | libc::EPERM as u32));
        let program = libc::sock_fprog {
            len: filter.len() as u16,
            filter: filter.as_mut_ptr(),
        };
        // SAFETY: kernel copies the correct set of instructions before returning.
        if unsafe { libc::prctl(libc::PR_SET_SECCOMP, 2, &program, 0, 0) } != 0 {
            return Err(FilterError::Kernel(io::Error::last_os_error()));
        }
        Ok(())
    }
}
