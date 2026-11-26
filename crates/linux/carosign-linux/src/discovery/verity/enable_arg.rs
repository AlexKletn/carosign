#[repr(C)]
pub(super) struct EnableArg {
    pub(super) version: u32,
    pub(super) hash_algorithm: u32,
    pub(super) block_size: u32,
    pub(super) salt_size: u32,
    pub(super) salt_ptr: u64,
    pub(super) sig_size: u32,
    pub(super) reserved1: u32,
    pub(super) sig_ptr: u64,
    pub(super) reserved2: [u64; 11],
}
