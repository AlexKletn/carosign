use super::EvidenceContext;
use super::Observed;
use super::ProtectedAnswer;
use super::Verdict;
use crate::material::EvidenceError;
use crate::material::EvidenceMaterialRef;
use crate::requirements::EvidenceRequirements;
use carosign_core::operation::context::BootInstant;

/// He's checking the evidence.
///
/// ⚠️ The checks are part of the contract: deadline, recipient, call, requirements, and then
/// the hardware. The expired proof doesn't reach the module at all.
pub fn verify(
    material: &EvidenceMaterialRef,
    context: &EvidenceContext,
    requirements: &EvidenceRequirements,
    now: BootInstant,
    observed: Observed,
) -> Verdict {
    // 1.Term.
    if context.deadline.is_expired_at(now) {
        return Verdict::Rejected(EvidenceError::Expired);
    }
    // 2. The material of this operation.
    if material.operation() != context.operation {
        return Verdict::Rejected(EvidenceError::MalformedEvidence);
    }
    // 3. Recipient.
    if observed.recipient != context.recipient.id() {
        return Verdict::Rejected(EvidenceError::WrongRecipient);
    }
    // 4. Challenge.
    if observed.challenge != context.challenge.id() {
        return Verdict::Rejected(EvidenceError::ChallengeMismatch);
    }
    if !context.challenge.is_single_use() {
        return Verdict::Rejected(EvidenceError::ChallengeMismatch);
    }
    // 5 Requirements are agreed with the purpose.
    if requirements.validate().is_err() || requirements.purpose != context.purpose {
        return Verdict::Rejected(EvidenceError::IncompatibleAssurance);
    }
    // 6. What's the secure module saying?
    match observed.protected {
        ProtectedAnswer::Unavailable => {
            return Verdict::Rejected(EvidenceError::VerifierUnavailable);
        }
        ProtectedAnswer::Unverifiable => {
            return Verdict::Rejected(EvidenceError::UnverifiedProvenance);
        }
        ProtectedAnswer::Rejected => {
            return Verdict::Rejected(EvidenceError::UnverifiedProvenance);
        }
        ProtectedAnswer::Verified => {}
    }

    if requirements.hardware_required && !observed.hardware_path {
        return Verdict::Rejected(EvidenceError::IncompatibleAssurance);
    }
    // Observed fields are available to the caller and do not confirm the origin, one-time
    // call consumption, or hardware trust chain.
    Verdict::Rejected(EvidenceError::UnverifiedProvenance)
}
