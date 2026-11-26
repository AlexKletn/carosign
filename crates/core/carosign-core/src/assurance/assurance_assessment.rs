use super::AssuranceClass;
use crate::ids::AuthenticatorId;
use crate::ids::EnrollmentId;
use crate::ids::EvidenceId;
use crate::ids::OperationId;
use core::fmt;

/// Evaluation of the inspector: what is checked, by whom and within what limits.
///
/// ⚠️ Designer closed: the instance can only appear inside the kreit through explicit
/// issuance. provider manifest, worker response and line `evidence_format_id` They don't
/// turn on their own.
#[derive(Clone)]
pub struct AssuranceAssessment {
    evidence: EvidenceId,
    class: AssuranceClass,
    operation: OperationId,
    enrollment: EnrollmentId,
    authenticator: AuthenticatorId,
}

impl AssuranceAssessment {
    /// Records the assessment received by a trusted auditor.
    ///
    /// ⚠️ It's only suitable for incarnations of the verifier inside the trusted part of the
    /// platform. `HardwareVerified` Here is an honest statement about the inspection, not a
    /// desire to inspire trust.
    pub fn record(
        evidence: EvidenceId,
        class: AssuranceClass,
        operation: OperationId,
        enrollment: EnrollmentId,
        authenticator: AuthenticatorId,
    ) -> Self {
        Self {
            evidence,
            class,
            operation,
            enrollment,
            authenticator,
        }
    }

    /// What proof.
    pub fn evidence(&self) -> EvidenceId {
        self.evidence
    }

    /// What a class guarantee.
    pub fn class(&self) -> AssuranceClass {
        self.class
    }

    /// For what operation.
    pub fn operation(&self) -> OperationId {
        self.operation
    }

    /// What registration?
    pub fn enrollment(&self) -> EnrollmentId {
        self.enrollment
    }

    pub fn authenticator(&self) -> AuthenticatorId {
        self.authenticator
    }

    /// Is it suitable for an operation that changes the protected keys?
    ///
    /// The rule is, the software matching **not* becomes a hardware guarantee against being
    /// wrapped in this type. The default value is failure.
    pub fn satisfies_key_operation(&self) -> bool {
        matches!(self.class, AssuranceClass::HardwareVerified)
    }
}

impl fmt::Debug for AssuranceAssessment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssuranceAssessment")
            .field("class", &self.class)
            .finish_non_exhaustive()
    }
}
