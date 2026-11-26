use std::ffi::{CStr, CString, c_char, c_int, c_void};

use crate::result::PamCode;

use super::authentication::authenticate_user;

/// Answer: `pam_sm_authenticate`.
///
/// ⚠️ No change. `PAM_AUTHTOK`: key release - **not** part `carosign.client.v1` (Project 07) is
/// not used here.
pub fn authenticate_outcome(code: PamCode) -> i32 {
    code.raw()
}

/// PAM entry point: The PAM token is unchanged; service failure allows for a backup entry
/// method if it is provided by the PAM system stack.
#[unsafe(no_mangle)]
pub extern "C" fn pam_sm_authenticate(
    pamh: *mut core::ffi::c_void,
    flags: core::ffi::c_int,
    _argc: core::ffi::c_int,
    _argv: *const *const core::ffi::c_char,
) -> core::ffi::c_int {
    std::panic::catch_unwind(|| authenticate(pamh, flags))
        .unwrap_or(PamCode::AuthInfoUnavail)
        .raw()
}

/// Biometric factor verification does not create separate PAM credentials.
/// GDM calls setcred after a successful check, so the absence of this entry point should not
/// turn an already verified result into a module error.
#[unsafe(no_mangle)]
pub extern "C" fn pam_sm_setcred(
    _pamh: *mut core::ffi::c_void,
    _flags: core::ffi::c_int,
    _argc: core::ffi::c_int,
    _argv: *const *const core::ffi::c_char,
) -> core::ffi::c_int {
    PamCode::Success.raw()
}

fn authenticate(pamh: *mut c_void, flags: c_int) -> PamCode {
    if pamh.is_null() {
        return PamCode::AuthInfoUnavail;
    }
    // SAFETY: The PAM library is already loaded when the module is called, and the symbol is
    // searched explicitly in it so that ancillary programs can use rlib without PAM.
    let library =
        unsafe { libc::dlopen(c"libpam.so.0".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if library.is_null() {
        return PamCode::AuthInfoUnavail;
    }
    // SAFETY: The name of the symbol ends with zero, the library descriptor is alive.
    let symbol = unsafe { libc::dlsym(library, c"pam_get_user".as_ptr()) };
    let result = if symbol.is_null() {
        PamCode::AuthInfoUnavail
    } else {
        // SAFETY: The signature corresponds to the public ABI. pam_get_user.
        let get_user: unsafe extern "C" fn(
            *mut c_void,
            *mut *const c_char,
            *const c_char,
        ) -> c_int = unsafe { core::mem::transmute(symbol) };
        let mut user = core::ptr::null();
        // SAFETY: pamh is derived from libpam; the user points to a live cell.
        if unsafe { get_user(pamh, &mut user, core::ptr::null()) } != 0 || user.is_null() {
            PamCode::AuthInfoUnavail
        } else {
            // SAFETY: Libpam guarantees a string until the current call is completed.
            match unsafe { CStr::from_ptr(user) }.to_str() {
                Ok(user) => {
                    type GetItem =
                        unsafe extern "C" fn(*const c_void, c_int, *mut *const c_void) -> c_int;
                    let get_item = unsafe { libc::dlsym(library, c"pam_get_item".as_ptr()) };
                    let mut service = core::ptr::null();
                    let service = if get_item.is_null() {
                        None
                    } else {
                        let get_item: GetItem = unsafe { core::mem::transmute(get_item) };
                        if unsafe { get_item(pamh, 1, &mut service) } == 0 && !service.is_null() {
                            unsafe { CStr::from_ptr(service.cast()) }.to_str().ok()
                        } else {
                            None
                        }
                    };
                    let localizer =
                        std::sync::Arc::new(super::localization::for_call(library, pamh));
                    authenticate_user(
                        user,
                        if flags & 0x8000 != 0 {
                            ""
                        } else {
                            service.unwrap_or("")
                        },
                        &localizer,
                        || {
                            if flags & 0x8000 != 0 {
                                return;
                            }
                            let prompt = unsafe { libc::dlsym(library, c"pam_prompt".as_ptr()) };
                            if !prompt.is_null() {
                                type PamPrompt = unsafe extern "C" fn(
                                    *mut c_void,
                                    c_int,
                                    *mut *mut c_char,
                                    *const c_char,
                                    ...
                                )
                                    -> c_int;
                                // SAFETY: The symbol has a documented signature of libpam.
                                let prompt: PamPrompt = unsafe { core::mem::transmute(prompt) };
                                // SAFETY: The pamh acts within the call, the strings are
                                // terminated by NUL.
                                let Ok(message) = CString::new(localizer.text("verify-prompt"))
                                else {
                                    return;
                                };
                                unsafe {
                                    prompt(
                                        pamh,
                                        4,
                                        core::ptr::null_mut(),
                                        c"%s".as_ptr(),
                                        message.as_ptr(),
                                    );
                                }
                            }
                        },
                    )
                }
                Err(_) => PamCode::AuthInfoUnavail,
            }
        }
    };
    // SAFETY: Every successful dlopen is closed after the call is completed.
    unsafe { libc::dlclose(library) };
    result
}

#[cfg(test)]
#[path = "entrypoints.tests.rs"]
mod tests;
