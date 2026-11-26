use core::fmt;

use carosign_protocol::limits::FRAME_MAX;

pub const PACKET_MAX: usize = FRAME_MAX;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameError {
    TooLarge { actual: usize },
    Empty,
    Truncated,
    DescriptorMismatch { declared: usize, received: usize },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { actual } => write!(f, "кадр {actual} байт больше {PACKET_MAX}"),
            Self::Empty => f.write_str("пустой кадр"),
            Self::Truncated => f.write_str("сообщение обрезано ядром"),
            Self::DescriptorMismatch { declared, received } => {
                write!(f, "дескрипторов: объявлено {declared}, пришло {received}")
            }
        }
    }
}

impl core::error::Error for FrameError {}

/// Set `truncated` from `MSG_TRUNC` or `MSG_CTRUNC`.
pub fn check(bytes: &[u8], truncated: bool) -> Result<(), FrameError> {
    if truncated {
        return Err(FrameError::Truncated);
    }
    if bytes.is_empty() {
        return Err(FrameError::Empty);
    }
    if bytes.len() > PACKET_MAX {
        return Err(FrameError::TooLarge {
            actual: bytes.len(),
        });
    }
    Ok(())
}

/// The caller owns the received descriptors and must close them on rejection.
pub fn check_descriptors(fd_slots: usize, received: usize) -> Result<(), FrameError> {
    if received > carosign_protocol::limits::FD_SLOTS_MAX {
        return Err(FrameError::DescriptorMismatch {
            declared: fd_slots,
            received,
        });
    }
    if fd_slots != received {
        return Err(FrameError::DescriptorMismatch {
            declared: fd_slots,
            received,
        });
    }
    Ok(())
}

#[path = "frame.tests.rs"]
#[cfg(test)]
mod tests;
