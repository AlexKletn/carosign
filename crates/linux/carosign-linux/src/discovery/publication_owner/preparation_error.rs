use std::io;

use crate::discovery::archive::ArchiveError;
use crate::discovery::signature_policy::PolicyError;

#[derive(Debug)]
pub(super) enum PreparationError {
    Io(io::Error),
    SignaturePolicy(PolicyError),
    Archive(ArchiveError),
    InvalidSupervisorPolicy,
    PolicyChanged,
    IdUnavailable,
}
