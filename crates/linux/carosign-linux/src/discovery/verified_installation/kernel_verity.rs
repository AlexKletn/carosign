use std::os::fd::{AsFd, OwnedFd};

use super::{Measure, Refusal};

pub(super) struct KernelVerity;
impl Measure for KernelVerity {
    fn matches(&self, file: &OwnedFd, expected: &[u8; 32]) -> Result<bool, Refusal> {
        crate::discovery::verity::matches_approved_digest(file.as_fd(), expected)
            .map_err(|_| Refusal::Unavailable)
    }
}
