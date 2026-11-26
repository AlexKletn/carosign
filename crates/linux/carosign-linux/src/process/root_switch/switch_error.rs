use super::SwitchStage;
use std::io;

#[derive(Debug)]
pub(crate) struct SwitchError {
    pub(crate) stage: SwitchStage,
    pub(crate) source: io::Error,
}
