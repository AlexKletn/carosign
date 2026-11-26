#[repr(C)]
pub(super) struct MountAttr {
    pub(super) attr_set: u64,
    pub(super) attr_clr: u64,
    pub(super) propagation: u64,
    pub(super) userns_fd: u64,
}
