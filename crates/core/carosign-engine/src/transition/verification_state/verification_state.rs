use super::VerificationDelivery;

/// Your state of check.
///
/// ⚠️ It carries a *link to proof*, not bytes: proof lives on limited delivery.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct VerificationState {
    /// Reference to the withheld evidence, if a match already existed.
    pub evidence: Option<carosign_core::evidence::EvidenceRef>,
    /// The state of a single record of a positive result.
    pub delivery: VerificationDelivery,
}
