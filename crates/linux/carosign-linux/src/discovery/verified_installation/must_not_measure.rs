use std::os::fd::OwnedFd;

use super::{Measure, Refusal};

pub(super) struct MustNotMeasure;
impl Measure for MustNotMeasure {
    fn matches(&self, _: &OwnedFd, _: &[u8; 32]) -> Result<bool, Refusal> {
        panic!("некорректная запись не должна открывать файлы")
    }
}
