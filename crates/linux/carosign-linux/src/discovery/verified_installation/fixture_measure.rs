use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;

use super::{Measure, Refusal};

pub(super) struct FixtureMeasure {
    pub(super) fail_inode: Option<u64>,
}

impl Measure for FixtureMeasure {
    fn matches(&self, file: &OwnedFd, expected: &[u8; 32]) -> Result<bool, Refusal> {
        let stat = std::fs::File::from(file.try_clone().unwrap())
            .metadata()
            .unwrap();
        Ok(expected == &[7; 32] && Some(stat.ino()) != self.fail_inode)
    }
}
