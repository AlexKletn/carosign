use crate::process::runtime_mount::MountError;
use std::io;

#[derive(Debug)]
pub(in crate::discovery) enum RuntimeError {
    InvalidName,
    DuplicateName,
    InvalidFile,
    Unapproved,
    Io(io::Error),
    Mount(MountError),
}
