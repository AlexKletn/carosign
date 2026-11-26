#[repr(C)]
pub(super) struct CapabilityHeader {
    pub(super) version: u32,
    pub(super) pid: i32,
}
