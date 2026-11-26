use super::NamespaceError;
use super::NamespaceStage;
use std::io;

/// Creates separate mount and network namespaces, and distributes mounts recursively private.
/// Call only in a separate single-threaded assistant until privileges are reset; any failure
/// must complete the process.
/// File system and network permissions are further limited later.
pub(crate) fn enter() -> Result<(), NamespaceError> {
    // SAFETY: unshare only changes the namespaces of the current process; the function is
    // intended exclusively for a single-threaded assistant.
    if unsafe { libc::unshare(libc::CLONE_NEWNS | libc::CLONE_NEWNET) } != 0 {
        return Err(NamespaceError {
            stage: NamespaceStage::Create,
            source: io::Error::last_os_error(),
        });
    }
    // SAFETY: The path is a static string; the rest of the pointers are not used when you
    // change the spread.
    if unsafe {
        libc::mount(
            std::ptr::null(),
            c"/".as_ptr(),
            std::ptr::null(),
            libc::MS_REC | libc::MS_PRIVATE,
            std::ptr::null(),
        )
    } != 0
    {
        return Err(NamespaceError {
            stage: NamespaceStage::PrivatePropagation,
            source: io::Error::last_os_error(),
        });
    }
    Ok(())
}
