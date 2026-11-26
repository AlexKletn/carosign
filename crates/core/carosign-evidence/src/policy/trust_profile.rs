use crate::requirements::EvidenceRequirements;
use carosign_core::assurance::AssuranceClass;

/// Trust profile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrustProfile {
    /// Normal local input: a program matcher is acceptable.
    LocalLogin,
    /// Secure key operation: hardware warranty only.
    KeyOperation,
}

impl TrustProfile {
    /// Profile requirements.
    pub fn requirements(self) -> EvidenceRequirements {
        match self {
            Self::LocalLogin => EvidenceRequirements::local_login(),
            Self::KeyOperation => EvidenceRequirements::key_operation(),
        }
    }

    /// Whether the guarantee class is good.
    pub fn accepts(self, class: AssuranceClass) -> bool {
        self.requirements().accepts(class)
    }
}
