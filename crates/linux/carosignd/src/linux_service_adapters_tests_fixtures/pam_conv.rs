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
