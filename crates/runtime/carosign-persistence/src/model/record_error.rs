use core::fmt;

/// Why is the record not good (form)?
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RecordError {
    /// An opaque link is longer than 4,096 bytes.
    OpaqueRefTooLong,
    /// There is no unchanging plan of change.
    MissingPlan,
    /// The goal generation does not coincide with an immutable plan.
    PlanGenerationMismatch,
}

impl fmt::Debug for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OpaqueRefTooLong => "RecordError::OpaqueRefTooLong",
            Self::MissingPlan => "RecordError::MissingPlan",
            Self::PlanGenerationMismatch => "RecordError::PlanGenerationMismatch",
        })
    }
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OpaqueRefTooLong => "непрозрачная ссылка длиннее 4096 байт",
            Self::MissingPlan => "нет плана изменения",
            Self::PlanGenerationMismatch => "поколение цели не совпадает с планом",
        })
    }
}

impl core::error::Error for RecordError {}
