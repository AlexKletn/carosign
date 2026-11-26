use std::os::fd::BorrowedFd;

pub(super) enum MountInput<'a> {
    Source(BorrowedFd<'a>),
    Tree(BorrowedFd<'a>),
}
