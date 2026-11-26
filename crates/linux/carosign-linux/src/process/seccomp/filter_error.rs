use std::io;

#[derive(Debug)]
pub(crate) enum FilterError {
    InvalidPolicy,
    UnsupportedArchitecture,
    NoNewPrivilegesRequired,
    Kernel(io::Error),
}
