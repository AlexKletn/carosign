use std::io;

/// Transportation failure. `WouldBlock` It means the recording hasn't started.
#[derive(Debug)]
pub enum TransportError {
    WouldBlock,
    Closed,
    Invalid,
    Io(io::Error),
}
