use super::PrivilegeError;
use super::PrivilegeStage;
use std::io;

/// Prohibits the promotion of privileges in the subsequent exec and reads the state back. A
/// refusal or unconfirmed result does not allow continuation.
pub(crate) fn set_no_new_privileges() -> Result<(), PrivilegeError> {
    // SAFETY: prctl is invoked for the current process with documented integer arguments;
    // pointers are not transmitted.
    let result = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if result != 0 {
        return Err(PrivilegeError {
            stage: PrivilegeStage::SetNoNewPrivileges,
            source: io::Error::last_os_error(),
        });
    }
    // SAFETY: Request the status of the current process without pointers.
    let result = unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) };
    if result != 1 {
        return Err(PrivilegeError {
            stage: PrivilegeStage::VerifyNoNewPrivileges,
            source: if result < 0 {
                io::Error::last_os_error()
            } else {
                io::Error::other("no_new_privs не подтверждён")
            },
        });
    }
    Ok(())
}
