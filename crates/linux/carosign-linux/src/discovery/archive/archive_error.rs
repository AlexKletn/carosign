use std::io;

#[derive(Debug)]
pub(in crate::discovery) enum ArchiveError {
    Io(io::Error),
    Unsafe,
    Limit,
    InvalidMetadata,
}

impl From<io::Error> for ArchiveError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
