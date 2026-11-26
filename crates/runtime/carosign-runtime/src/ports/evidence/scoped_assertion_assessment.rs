use super::ScopedAssertionContext;

/// The result of limited approval evaluation: The proof identifier must be issued by the
/// trusted owner of the platform record; the identifier of a specific registration is not
/// invented when matched across the entire set.
/// The type itself does not confirm the origin of the ID, nor the credibility of the
/// verifier.
pub struct ScopedAssertionAssessment {
    lease: carosign_evidence::store::AssertionLease,
    context: ScopedAssertionContext,
    class: carosign_core::assurance::AssuranceClass,
}

impl ScopedAssertionAssessment {
    /// Maintains the initial approval area along with the programmatic evaluation.
    /// Hardware warranty is not issued in this way.
    pub fn software(
        lease: carosign_evidence::store::AssertionLease,
        context: ScopedAssertionContext,
    ) -> Self {
        Self {
            lease,
            context,
            class: carosign_core::assurance::AssuranceClass::SoftwareAsserted,
        }
    }

    /// Returns the original statement without allowing repossession.
    pub fn assertion(&self) -> &carosign_evidence::store::ProviderAssertionRecord {
        self.lease.record()
    }

    /// Identifier of the internal record; its origin is checked separately.
    pub fn evidence(&self) -> carosign_core::ids::EvidenceId {
        carosign_core::ids::EvidenceId::from_bytes(self.lease.reference().id())
            .expect("владелец материала не выдаёт нулевой ID")
    }

    /// A link issued by the owner of a one-time statement record.
    pub fn reference(&self) -> &carosign_evidence::material::EvidenceMaterialRef {
        self.lease.reference()
    }

    /// The full area for which the assessment was made.
    pub fn context(&self) -> &ScopedAssertionContext {
        &self.context
    }

    /// Checks that the extradition has not been withdrawn and the deadline has not yet
    /// expired.
    pub fn still_valid(&self, now: carosign_core::operation::context::BootInstant) -> bool {
        self.lease.still_valid(now).is_ok()
    }

    /// Returns the evaluation program class.
    pub fn class(&self) -> carosign_core::assurance::AssuranceClass {
        self.class
    }
}
