use super::{pam_message::PamMessage, pam_response::PamResponse};

#[repr(C)]
pub(super) struct PamConv {
    pub(super) conv: Option<
        unsafe extern "C" fn(
            libc::c_int,
            *mut *const libc::c_void,
            *mut *mut libc::c_void,
            *mut libc::c_void,
        ) -> libc::c_int,
    >,
    pub(super) appdata_ptr: *mut libc::c_void,
}

pub(super) unsafe extern "C" fn converse(
    count: libc::c_int,
    messages: *mut *const libc::c_void,
    response: *mut *mut libc::c_void,
    data: *mut libc::c_void,
) -> libc::c_int {
    if count != 1 || messages.is_null() || response.is_null() || data.is_null() {
        return 19;
    }
    // SAFETY: Libpam transmits the count message array and our counter cell.
    let message = unsafe { &*((*messages).cast::<PamMessage>()) };
    if message.style != 4 || message.message.is_null() {
        return 19;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(message.message) };
    if text.to_bytes() != "Подтвердите биометрическую проверку".as_bytes()
    {
        return 19;
    }
    unsafe { *(data.cast::<usize>()) += 1 };
    // Libpam releases a response at the end of the conversation.
    let value = unsafe { libc::calloc(1, core::mem::size_of::<PamResponse>()) };
    if value.is_null() {
        return 19;
    }
    unsafe { *response = value };
    0
}
