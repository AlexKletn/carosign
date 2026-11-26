#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackupIdentity {
    pub(crate) device: u64,
    pub(crate) inode: u64,
    pub(crate) length: u64,
    pub(crate) digest: [u8; 32],
}
