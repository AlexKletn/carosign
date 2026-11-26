use super::IdentityError;

/// Allows a name via system NSS; not having a record is not an NSS error.
pub(super) fn account_uid(name: &str) -> Result<u32, IdentityError> {
    use std::ffi::CString;
    let name = CString::new(name).map_err(|_| IdentityError::UnknownSubject)?;
    let mut size = 1024usize;
    loop {
        let mut buffer = vec![0u8; size];
        // SAFETY: passwd is filled in getpwnam_r Before reading, the buffer and pointers are
        // valid for the time of the call, the returned pointers are not stored.
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result = std::ptr::null_mut();
        let error = unsafe {
            libc::getpwnam_r(
                name.as_ptr(),
                &mut entry,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if error == libc::ERANGE && size < 1024 * 1024 {
            size *= 2;
            continue;
        }
        if error != 0 {
            return Err(IdentityError::Unavailable);
        }
        return if result.is_null() {
            Err(IdentityError::UnknownSubject)
        } else {
            Ok(entry.pw_uid)
        };
    }
}
