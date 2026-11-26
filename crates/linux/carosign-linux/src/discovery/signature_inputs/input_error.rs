use super::super::signature_runtime;
use super::super::signature_status;
use std::io;

#[derive(Debug)]
pub(in crate::discovery) enum InputError {
    Io(io::Error),
    InvalidFile,
    TooLarge,
    KeyringUnapproved,
    VerifierUnapproved,
    Signature(signature_status::VerificationError),
    Runtime(signature_runtime::RuntimeError),
}

impl From<io::Error> for InputError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
