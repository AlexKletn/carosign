use super::{EnrollmentId, fmt};

/// It's not going to be a projection.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProjectionError {
    /// The old purpose has disappeared or replaced a generation.
    OldMissingOrStale {
        /// What.
        enrollment: EnrollmentId,
    },
    /// The old target of the wrong subject or authenticator.
    OldOutOfScope,
    /// Storage didn't match the plan.
    KindMismatch,
    /// The format didn't match the plan.
    FormatMismatch,
    /// The digest of the opaque reference did not match the plan.
    ReferenceMismatch,
    /// The tape's not working.
    NotActive,
    /// The picture of the area didn't match: wrong set or wrong size.
    ScopeMismatch,
    /// The plan does not correspond to the transition (for example, the replacement does not
    /// have an old goal).
    PlanShape,
    /// Removal: There is no old purpose in the plan.
    MissingOldTarget,
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OldMissingOrStale { enrollment } => {
                write!(
                    f,
                    "старая регистрация {enrollment:?} исчезла или сменила поколение"
                )
            }
            Self::OldOutOfScope => f.write_str("старая цель вне области изменения"),
            Self::KindMismatch => f.write_str("вид хранения не совпал с планом"),
            Self::FormatMismatch => f.write_str("формат не совпал с планом"),
            Self::ReferenceMismatch => f.write_str("дайджест ссылки не совпал с планом"),
            Self::NotActive => f.write_str("запись не действует"),
            Self::ScopeMismatch => f.write_str("снимок области не сошёлся с планом"),
            Self::PlanShape => f.write_str("план не сходится с переходом"),
            Self::MissingOldTarget => f.write_str("у удаления нет старой цели"),
        }
    }
}

impl core::error::Error for ProjectionError {}
