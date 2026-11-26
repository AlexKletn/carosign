use std::os::fd::OwnedFd;

use super::Refusal;

pub(super) trait Measure {
    fn matches(&self, file: &OwnedFd, expected: &[u8; 32]) -> Result<bool, Refusal>;
}
