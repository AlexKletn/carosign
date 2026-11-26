use std::os::fd::BorrowedFd;

/// A retained file already selected by a trusted installer.
/// This type itself does not verify the origin of the file or the authority of the
/// composition.
pub(crate) struct RuntimeFile<'a> {
    pub(crate) path: &'a str,
    pub(crate) source: BorrowedFd<'a>,
}
