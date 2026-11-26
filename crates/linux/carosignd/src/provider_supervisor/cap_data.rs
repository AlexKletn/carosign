#[repr(C)]
pub(super) struct CapData {
    pub(super) effective: u32,
    pub(super) permitted: u32,
    pub(super) inheritable: u32,
}
