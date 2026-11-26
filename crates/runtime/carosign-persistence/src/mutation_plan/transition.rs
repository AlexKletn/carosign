use super::PlanError;

/// What's planned.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Transition {
    EnrollNew,
    /// Replace the existing one.
    EnrollReplace,
    /// Removal.
    Remove,
}

impl Transition {
    /// The value of the wire.
    pub fn wire(self) -> i32 {
        match self {
            Self::EnrollNew => 1,
            Self::EnrollReplace => 2,
            Self::Remove => 3,
        }
    }

    /// The meaning of the wire.
    ///
    /// ⚠️ Zero and unfamiliar is rejection.
    pub fn from_wire(value: i32) -> Result<Self, PlanError> {
        Ok(match value {
            1 => Self::EnrollNew,
            2 => Self::EnrollReplace,
            3 => Self::Remove,
            _ => return Err(PlanError::UnknownTransition { value }),
        })
    }

    /// Do you need a new ID?
    pub fn creates_enrollment(self) -> bool {
        matches!(self, Self::EnrollNew | Self::EnrollReplace)
    }
}
