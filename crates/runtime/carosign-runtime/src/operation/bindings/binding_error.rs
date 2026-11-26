use core::fmt;

/// That's not good for a bundle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindingError {
    /// The provider’s link is longer than the limit.
    ReferenceTooLarge {
        actual: usize,
    },
    /// The format is longer than the limit.
    FormatTooLong,
    /// For storage, a link is not necessary, and for an opaque one, it is mandatory.
    KindMismatch,
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReferenceTooLarge { actual } => {
                write!(f, "ссылка провайдера {actual} байт длиннее предела")
            }
            Self::FormatTooLong => f.write_str("идентификатор формата длиннее 128 байт"),
            Self::KindMismatch => f.write_str("вид связки не сходится со ссылкой"),
        }
    }
}

impl core::error::Error for BindingError {}
