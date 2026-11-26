use super::MountStage;
use std::io;

#[derive(Debug)]
pub(crate) struct MountError {
    pub(crate) stage: MountStage,
    pub(crate) source: io::Error,
}
