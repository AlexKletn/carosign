#[repr(C)]
pub(super) struct PamMessage {
    pub(super) style: libc::c_int,
    pub(super) message: *const libc::c_char,
}
