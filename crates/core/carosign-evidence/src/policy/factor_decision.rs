use super::TrustProfile;
use carosign_core::assurance::AssuranceClass;
use carosign_core::operation::context::VerificationPurpose;

/// Admission decision.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FactorDecision {
    /// Factor accepted for this request.
    Accepted,
    /// Refusal.
    Refused,
}

/// Does politics take a factor?
///
/// ⚠️ For `KeyOperation` only `HardwareVerified`regardless of the profile: `LocalLogin` It
/// doesn't apply at all.
pub fn decide(
    purpose: VerificationPurpose,
    profile: TrustProfile,
    class: AssuranceClass,
) -> FactorDecision {
    // Profile defines not only the class, but also the target, and even the hardware score
    // doesn't allow you to transfer the login profile decision to the key operation.
    if profile.requirements().purpose != purpose {
        return FactorDecision::Refused;
    }
    if purpose.requires_hardware_assurance() && class != AssuranceClass::HardwareVerified {
        return FactorDecision::Refused;
    }
    if profile.accepts(class) {
        FactorDecision::Accepted
    } else {
        FactorDecision::Refused
    }
}
