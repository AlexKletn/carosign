use super::FileRole;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct ApprovedFile {
    pub(super) role: FileRole,
    pub(super) digest: [u8; 32],
}
