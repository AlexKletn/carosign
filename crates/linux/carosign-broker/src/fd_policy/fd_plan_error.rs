use crate::resources::ClaimError;
use core::fmt;

/// Plan failure.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FdPlanError {
    /// The number didn't match.
    CountMismatch {
        /// Announced.
        declared: usize,
        received: usize,
    },
    /// The role didn't match.
    RoleMismatch,
    /// Issuance does not pass verification.
    Claim(ClaimError),
}

impl fmt::Display for FdPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CountMismatch { declared, received } => {
                write!(f, "дескрипторов: объявлено {declared}, пришло {received}")
            }
            Self::RoleMismatch => f.write_str("роль не совпала со слотом"),
            Self::Claim(error) => write!(f, "выдача неверна: {error}"),
        }
    }
}

impl core::error::Error for FdPlanError {}

impl From<ClaimError> for FdPlanError {
    fn from(value: ClaimError) -> Self {
        Self::Claim(value)
    }
}
