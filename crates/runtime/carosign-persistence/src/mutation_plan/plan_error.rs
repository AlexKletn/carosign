use core::fmt;

/// The plan's wrong.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlanError {
    /// It's an unfamiliar kind of transition.
    UnknownTransition {
        value: i32,
    },
    /// Registration without a new number or removal with it.
    TransitionShape,
    /// Generation is not: zero at registration or zero at removal.
    GenerationShape,
    /// There's too many pictures.
    TooMany {
        actual: usize,
    },
    /// Pictures are not sorted or repeated.
    Unsorted,
    /// The image does not belong to the area (another subject or apparatus).
    OutOfScope,
    /// The format is longer than the limit.
    FormatTooLong,
    /// The string is not UTF-8 or contains control signs.
    BadFormat,
    /// The digest did not match the canonical record.
    DigestMismatch,
    /// Unfamiliar storage.
    UnknownBindingKind {
        value: i32,
    },
    /// The old goal doesn't fit with the transition.
    OldTargetShape,
    /// The new registration coincides with the old one.
    SameEnrollment,
    /// The semantic plan is not consistent with the record.
    InvalidSemanticPlan,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTransition { value } => write!(f, "незнакомый переход {value}"),
            Self::TransitionShape => f.write_str("форма перехода не сходится с номером"),
            Self::GenerationShape => f.write_str("поколение не сходится с переходом"),
            Self::TooMany { actual } => write!(f, "снимков {actual} — больше предела"),
            Self::Unsorted => f.write_str("снимки не отсортированы или повторяются"),
            Self::OutOfScope => f.write_str("снимок вне области изменения"),
            Self::FormatTooLong => f.write_str("идентификатор формата длиннее 128 байт"),
            Self::BadFormat => f.write_str("идентификатор формата негоден"),
            Self::DigestMismatch => f.write_str("дайджест не совпал с канонической записью"),
            Self::UnknownBindingKind { value } => write!(f, "незнакомый вид хранения {value}"),
            Self::OldTargetShape => f.write_str("старая цель не сходится с переходом"),
            Self::SameEnrollment => f.write_str("новая регистрация совпадает со старой"),
            Self::InvalidSemanticPlan => f.write_str("семантический план не согласуется с записью"),
        }
    }
}

impl core::error::Error for PlanError {}
