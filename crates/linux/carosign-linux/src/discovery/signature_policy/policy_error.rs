use super::super::signature_inputs::InputError;
use super::super::signature_runtime::RuntimeError;
use std::io;

#[derive(Debug)]
pub(in crate::discovery) enum PolicyError {
    Io(io::Error),
    Invalid,
    Changed,
    Input(InputError),
    Runtime(RuntimeError),
}
