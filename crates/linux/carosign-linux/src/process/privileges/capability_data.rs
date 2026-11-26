#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct CapabilityData {
    pub(super) effective: u32,
    pub(super) permitted: u32,
    pub(super) inheritable: u32,
}
