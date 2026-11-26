use std::io;
use std::os::fd::OwnedFd;
use std::time::Duration;

use super::{ExecReport, read_until::read_until};

pub(crate) struct ErrorReader(pub(super) OwnedFd);

impl ErrorReader {
    pub(crate) fn read(self, timeout: Duration) -> io::Result<ExecReport> {
        let mut message = [0; 5];
        let count = read_until(&self.0, &mut message, timeout)?;
        match count {
            0 => Ok(ExecReport::ClosedWithoutReport),
            5 => Ok(ExecReport::Failure {
                stage: message[0],
                errno: i32::from_be_bytes(message[1..].try_into().expect("четыре байта")),
            }),
            _ => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated error report",
            )),
        }
    }
}
