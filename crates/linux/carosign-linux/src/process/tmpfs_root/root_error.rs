use super::RootStage;
use std::io;

#[derive(Debug)]
pub(crate) struct RootError {
    pub(crate) stage: RootStage,
    pub(crate) source: io::Error,
}
