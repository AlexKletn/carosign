use super::PlanError;

/// Type of storage of the bundle (document 34).
///
/// ⚠️ Mandatory ** and ** in the registry record, ** and ** in the plan shot: an empty link
/// from the provider ** not ** means storage (it is forbidden expressly), zero and unfamiliar
/// - refusal.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StoredBindingKind {
    /// ISP link.
    ProviderOpaque,
    /// Through the authorized vault.
    ScopedVault,
}

impl StoredBindingKind {
    /// The value of the wire (coincides with the `EnrollmentBindingKind`).
    pub fn wire(self) -> i32 {
        match self {
            Self::ProviderOpaque => 1,
            Self::ScopedVault => 2,
        }
    }

    pub fn from_wire(value: i32) -> Result<Self, PlanError> {
        Ok(match value {
            1 => Self::ProviderOpaque,
            2 => Self::ScopedVault,
            _ => return Err(PlanError::UnknownBindingKind { value }),
        })
    }
}
