use super::Provenance;
use carosign_core::assurance::AssuranceAssessment;
use carosign_core::assurance::AssuranceClass;
use core::fmt;

/// Estimate with origin.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Assessment {
    /// Guarantee class.
    pub class: AssuranceClass,
    /// What's mined.
    pub provenance: Provenance,
}

impl Assessment {
    /// Approval of a provider without a token.
    ///
    /// ⚠️ Class-- `SoftwareAsserted`: There was no hardware path.
    /// A separate origin is necessary only in order to distinguish in the journal and in the
    /// diagnosis "the provider said" from "we calculated ourselves."
    pub fn provider_assertion() -> Self {
        Self {
            class: AssuranceClass::SoftwareAsserted,
            provenance: Provenance::ProviderAssertion,
        }
    }

    /// Programme evaluation.
    pub fn software() -> Self {
        Self {
            class: AssuranceClass::SoftwareAsserted,
            provenance: Provenance::SoftwareMatcher,
        }
    }

    /// Hardware evaluation.
    pub fn hardware() -> Self {
        Self {
            class: AssuranceClass::HardwareVerified,
            provenance: Provenance::ProtectedModule,
        }
    }

    /// Translates it into a domain score.
    ///
    /// Wrapping a software assessment in this type does not increase its assurance class.
    pub fn into_domain(
        self,
        evidence: carosign_core::ids::EvidenceId,
        operation: carosign_core::ids::OperationId,
        enrollment: carosign_core::ids::EnrollmentId,
        authenticator: carosign_core::ids::AuthenticatorId,
    ) -> AssuranceAssessment {
        AssuranceAssessment::record(evidence, self.class, operation, enrollment, authenticator)
    }
}

impl fmt::Display for Assessment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} из {:?}", self.class, self.provenance)
    }
}
