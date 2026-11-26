#[repr(C)]
pub(super) struct PamResponse {
    response: *mut libc::c_char,
    code: libc::c_int,
}
