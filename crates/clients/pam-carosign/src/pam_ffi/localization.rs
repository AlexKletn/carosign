use std::ffi::{CStr, CString, c_char, c_void};

use carosign_l10n::{Localizer, Preferences};

pub(super) fn for_call(library: *mut c_void, pamh: *mut c_void) -> Localizer {
    type GetEnv = unsafe extern "C" fn(*mut c_void, *const c_char) -> *const c_char;
    // SAFETY: The caller holds the libpam handle open throughout authentication.
    let symbol = unsafe { libc::dlsym(library, c"pam_getenv".as_ptr()) };
    let get_env: Option<GetEnv> = if symbol.is_null() {
        None
    } else {
        // SAFETY: The symbol is the documented libpam pam_getenv entry point.
        Some(unsafe { std::mem::transmute(symbol) })
    };
    let preferences = Preferences::from_variables(|name| {
        if let Some(get_env) = get_env {
            let key = CString::new(name).ok()?;
            // SAFETY: pamh and key remain valid for the call. Libpam owns the returned string.
            let value = unsafe { get_env(pamh, key.as_ptr()) };
            if !value.is_null() {
                // SAFETY: libpam returns a NUL-terminated string, which is copied here.
                if let Ok(value) = unsafe { CStr::from_ptr(value) }.to_str() {
                    return Some(value.to_owned());
                }
            }
        }
        std::env::var(name).ok()
    });
    Localizer::system(preferences)
}
