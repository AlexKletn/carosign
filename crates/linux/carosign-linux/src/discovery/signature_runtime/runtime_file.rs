use std::os::fd::BorrowedFd;

/// One file is only in the unpublished root of the individual assistant.
pub(in crate::discovery) struct RuntimeFile<'a> {
    pub name: &'a str,
    pub source: BorrowedFd<'a>,
    pub approved_verity: [u8; 32],
}
