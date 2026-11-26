use std::ffi::{CStr, CString};

pub(super) fn lookup_user(name: Option<&str>, uid: u32) -> Option<(String, u32)> {
    let name = name.map(CString::new).transpose().ok()?;
    let mut buffer = vec![0u8; 16384];
    // SAFETY: All NSS arguments remain valid throughout the call. The strings and
    // structure are copied before the buffer is freed.
    unsafe {
        let mut pwd: libc::passwd = core::mem::zeroed();
        let mut result = core::ptr::null_mut();
        let rc = match name {
            Some(ref name) => libc::getpwnam_r(
                name.as_ptr(),
                &mut pwd,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            ),
            None => libc::getpwuid_r(
                uid,
                &mut pwd,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            ),
        };
        if rc != 0 || result.is_null() || pwd.pw_name.is_null() {
            return None;
        }
        Some((
            CStr::from_ptr(pwd.pw_name).to_str().ok()?.to_owned(),
            pwd.pw_uid,
        ))
    }
}
