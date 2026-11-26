use super::RequirementConflict;
use carosign_core::assurance::AssuranceClass;
use carosign_core::operation::context::VerificationPurpose;

/// Which is mandatory.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvidenceRequirements {
    /// The purpose for which the factor is taken.
    pub purpose: VerificationPurpose,
    /// Do you need a hardware warranty?
    pub hardware_required: bool,
    /// Need a fresh call?
    pub fresh_challenge: bool,
}

impl EvidenceRequirements {
    /// Requirements for normal local entry.
    pub fn local_login() -> Self {
        Self {
            purpose: VerificationPurpose::SessionLogin,
            hardware_required: false,
            fresh_challenge: true,
        }
    }

    /// Requirements for a secure key operation.
    ///
    /// ⚠️ Here hardware warranty is mandatory: software matching is not suitable here, and
    /// this is not a setup.
    pub fn key_operation() -> Self {
        Self {
            purpose: VerificationPurpose::KeyOperation,
            hardware_required: true,
            fresh_challenge: true,
        }
    }

    /// Is the class of guarantee suitable for the requirements?
    pub fn accepts(&self, class: AssuranceClass) -> bool {
        // Contradictory requirements cannot even resolve a programmatic result if the caller
        // forgot to call validate() separately.
        if self.validate().is_err() {
            return false;
        }
        match (self.hardware_required, class) {
            (true, AssuranceClass::HardwareVerified) => true,
            (true, _) => false,
            (false, AssuranceClass::Unverified) => false,
            (false, _) => true,
        }
    }
}

impl EvidenceRequirements {
    /// Whether the requirements are aligned with the purpose.
    ///
    /// ⚠️ For the operation with the key, the hardware warranty ** is obliged to be a
    /// requirement: otherwise, the verifier will accept the software match, and this is no
    /// longer a “weak setup”, but a different model of trust.
    pub fn validate(&self) -> Result<(), RequirementConflict> {
        if self.purpose.requires_hardware_assurance() && !self.hardware_required {
            return Err(RequirementConflict);
        }
        Ok(())
    }
}
