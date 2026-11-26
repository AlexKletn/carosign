#[repr(C)]
pub(super) struct Digest {
    pub(super) algorithm: u16,
    pub(super) size: u16,
    pub(super) bytes: [u8; 32],
}
