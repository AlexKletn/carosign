use core::fmt;

/// What's not good.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BodyError {
    /// Displacement is not zero (reserved for the future).
    OffsetNotSupported {
        /// What.
        offset: u64,
    },
    /// Length is off limits.
    BadLength {
        length: u64,
    },
    /// The size of the file does not match the length.
    SizeMismatch {
        length: u64,
        file_size: u64,
    },
    /// The hash is not that long.
    BadDigestLength {
        actual: usize,
    },
    /// The content didn't match the hash.
    DigestMismatch,
    /// The descriptor is not the same: changeable or unsealed.
    NotSealed,
    /// The descriptors are not exactly the same.
    WrongFdCount {
        actual: usize,
    },
    /// Body and `payload` in the same call.
    BothBodies,
}

impl fmt::Display for BodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OffsetNotSupported { offset } => {
                write!(f, "смещение {offset} не поддерживается")
            }
            Self::BadLength { length } => write!(f, "длина тела {length} вне пределов"),
            Self::SizeMismatch { length, file_size } => {
                write!(f, "длина {length} не сходится с размером файла {file_size}")
            }
            Self::BadDigestLength { actual } => write!(f, "хеш длиной {actual} байт"),
            Self::DigestMismatch => f.write_str("содержимое не совпало с хешем"),
            Self::NotSealed => f.write_str("дескриптор не запечатан"),
            Self::WrongFdCount { actual } => write!(f, "дескрипторов {actual}, а нужен ровно один"),
            Self::BothBodies => f.write_str("тело и payload в одном вызове"),
        }
    }
}

impl core::error::Error for BodyError {}
