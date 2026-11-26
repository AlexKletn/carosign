use super::RecordError;

/// Status of the record (`carosign.storage.v1.EnrollmentStatus`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EnrollmentStatus {
    /// It works.
    Active,
    /// Waiting for a check.
    PendingReconciliation,
    Unavailable,
}

impl EnrollmentStatus {
    /// The value of the wire.
    pub fn wire(self) -> i32 {
        match self {
            Self::Active => 1,
            Self::PendingReconciliation => 2,
            Self::Unavailable => 3,
        }
    }

    /// The meaning of the wire.
    ///
    /// ⚠️ Zero and the unfamiliar are refusals, not “consider them to be valid.”
    pub fn from_wire(value: i32) -> Result<Self, RecordError> {
        Ok(match value {
            1 => Self::Active,
            2 => Self::PendingReconciliation,
            3 => Self::Unavailable,
            _ => return Err(RecordError::UnknownStatus { value }),
        })
    }
}
