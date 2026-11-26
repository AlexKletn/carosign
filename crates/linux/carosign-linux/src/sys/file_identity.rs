/// Check if it is a regular file and who owns it.
///
/// ⚠️ To start an worker: a link, device or directory - a refusal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileIdentity {
    /// Regular file (not a link, not a directory, not a device).
    pub regular: bool,
    /// Owner's UID.
    pub uid: u32,
    /// Is there a record for "everyone."
    pub world_writable: bool,
    /// Device and inode (for reconciliation before and after inspection).
    pub device: u64,
    pub inode: u64,
}
