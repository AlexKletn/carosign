use super::NamespaceStage;
use std::io;

/// The error does not allow the launch to continue.
#[derive(Debug)]
pub(crate) struct NamespaceError {
    pub(crate) stage: NamespaceStage,
    pub(crate) source: io::Error,
}
