use super::PrivilegeStage;
use std::io;

#[derive(Debug)]
pub(crate) struct PrivilegeError {
    pub(crate) stage: PrivilegeStage,
    pub(crate) source: io::Error,
}
