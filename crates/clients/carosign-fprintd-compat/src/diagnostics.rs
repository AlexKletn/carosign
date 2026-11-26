//! Optional diagnostics that omit user data.

use std::io::Write;
use std::sync::OnceLock;

/// Records only a predefined event name and accepts no request data.
pub(crate) fn event(name: &'static str) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| {
        std::env::var_os("CAROSIGN_FPRINT_DIAGNOSTICS").as_deref() == Some(std::ffi::OsStr::new("1"))
    }) {
        return;
    }
    // SAFETY: The timespec points to valid writable memory.
    let mut time = unsafe { std::mem::zeroed::<libc::timespec>() };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return;
    }
    let _ = writeln!(
        std::io::stderr().lock(),
        "fprintd-diag monotonic={}.{:09} event={name}",
        time.tv_sec,
        time.tv_nsec
    );
}
