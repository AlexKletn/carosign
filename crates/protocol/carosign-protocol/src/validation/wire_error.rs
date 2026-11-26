use core::fmt;

use crate::limits;

/// What a message is not good for.
///
/// These local errors can contain internal details. Send public `ErrorCode` values to the
/// peer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireError {
    /// The frame is longer than 64 KB.
    FrameTooLarge {
        actual: usize,
    },
    /// The load is longer than the limit.
    FieldTooLong {
        field: &'static str,
        actual: usize,
        allowed: usize,
    },
    /// The ID is empty or not 16 bytes.
    BadId {
        field: &'static str,
    },
    /// The listing remains at zero, where it is prohibited.
    UnspecifiedEnum {
        field: &'static str,
    },
    /// The meaning of the listing is unknown to us.
    UnknownEnum {
        field: &'static str,
        /// What a value.
        value: i32,
    },
    /// That's not a phase for a message like that.
    UnexpectedPayload {
        /// That came.
        what: &'static str,
    },
    /// The frame number isn't next.
    BadFrameSequence {
        expected: u64,
        actual: u64,
    },
    /// It's not compatible.
    UnsupportedVersion {
        major: u32,
        minor: u32,
    },
    /// The descriptors are not matched with the frame.
    FdMismatch {
        /// How many slots are announced.
        declared: usize,
        received: usize,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameTooLarge { actual } => {
                write!(f, "кадр {actual} байт превышает {}", limits::FRAME_MAX)
            }
            Self::FieldTooLong {
                field,
                actual,
                allowed,
            } => write!(f, "поле {field}: {actual} при пределе {allowed}"),
            Self::BadId { field } => write!(f, "идентификатор {field} пуст или не 16 байт"),
            Self::UnspecifiedEnum { field } => write!(f, "перечисление {field} не задано"),
            Self::UnknownEnum { field, value } => {
                write!(f, "перечисление {field} имеет неизвестное значение {value}")
            }
            Self::UnexpectedPayload { what } => write!(f, "в этой фазе не бывает {what}"),
            Self::BadFrameSequence { expected, actual } => {
                write!(f, "номер кадра: ждали {expected}, пришло {actual}")
            }
            Self::UnsupportedVersion { major, minor } => {
                write!(f, "версия {major}.{minor} не поддерживается")
            }
            Self::FdMismatch { declared, received } => {
                write!(f, "дескрипторов: объявлено {declared}, пришло {received}")
            }
        }
    }
}

impl core::error::Error for WireError {}
