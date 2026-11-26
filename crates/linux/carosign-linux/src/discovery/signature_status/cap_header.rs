#[repr(C)]
pub(super) struct CapHeader {
    pub(super) version: u32,
    pub(super) pid: i32,
}
