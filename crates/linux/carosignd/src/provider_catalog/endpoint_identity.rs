use std::os::unix::fs::MetadataExt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct EndpointIdentity {
    device: u64,
    inode: u64,
    change_time: (i64, i64),
    pid: u32,
}

impl EndpointIdentity {
    pub(super) fn of(metadata: &std::fs::Metadata, pid: u32) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            change_time: (metadata.ctime(), metadata.ctime_nsec()),
            pid,
        }
    }
}
