use carosign_core::enrollment::EnrollmentGeneration;
use carosign_core::ids::EnrollmentId;
use carosign_core::ids::EvidenceId;
use core::fmt;

/// Checked status of registration and **how ** it is known.
///
/// ⚠️ Not. `bool`A yes/no statement without origin is a statement without a source.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VerifiedState {
    /// What exactly was checked.
    pub enrollment: EnrollmentId,
    /// A generation of verified records.
    pub generation: EnrollmentGeneration,
    /// What is proved is a reference to the reconciliation material.
    pub provenance: EvidenceId,
}

impl fmt::Debug for VerifiedState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VerifiedState(.., generation={})",
            self.generation.value()
        )
    }
}
