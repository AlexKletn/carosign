use crate::sys::LaunchMetadata;

pub(super) fn protected(meta: &LaunchMetadata, owner: u32, directory: bool) -> bool {
    meta.identity.uid == owner
        && meta.mode & (libc::S_IWGRP | libc::S_IWOTH) == 0
        && meta.mode & libc::S_IFMT
            == if directory {
                libc::S_IFDIR
            } else {
                libc::S_IFREG
            }
}
