//! Independent evaluation of a limited programmatic approval.
//!
//! In Linux, trust is based on system policy and a secure provider process, and the hardware
//! properties are not supported by this.

use carosign_core::assurance::AssuranceClass;
use carosign_core::ids::IdSource;
use carosign_evidence::requirements::EvidenceRequirements;
use carosign_evidence::store::EvidenceStore;
use carosign_runtime::ports::{
    BoxFuture, EvidenceContext, EvidencePort, EvidenceRefusal, ScopedAssertionAssessment,
    ScopedAssertionContext, TimeSource,
};
use carosign_runtime::provider::session::ScopedProviderAssertion;

use crate::scoped_challenge::ScopedChallenges;

pub(crate) fn evaluate(
    assertion: ScopedProviderAssertion,
    context: &ScopedAssertionContext,
    requirements: &EvidenceRequirements,
    sources: &Sources<'_>,
) -> Result<ScopedAssertionAssessment, EvidenceRefusal> {
    if context.purpose != requirements.purpose
        || !requirements.accepts(AssuranceClass::SoftwareAsserted)
    {
        return Err(EvidenceRefusal::Unverified);
    }
    let now = sources.clock.now();
    if context.deadline.is_expired_at(now) {
        return Err(EvidenceRefusal::Expired);
    }
    sources.trust.confirm(&assertion, context)?;
    // Destruction of the right to repeat does not roll back in the event of a subsequent
    // error.
    sources
        .challenges
        .consume(assertion.operation(), context, sources.clock.now())
        .map_err(|_| EvidenceRefusal::Unverified)?;
    sources.trust.confirm(&assertion, context)?;
    let record = assertion
        .into_evidence_record(context)
        .map_err(|_| EvidenceRefusal::Unverified)?;
    let now = sources.clock.now();
    let reference = sources
        .store
        .hold_assertion(sources.ids, record, now)
        .map_err(|_| EvidenceRefusal::Unavailable)?;
    match sources.store.take_assertion(reference.clone(), now) {
        Ok(lease) => Ok(ScopedAssertionAssessment::software(lease, context.clone())),
        Err(_) => {
            // An error after recording should not leave available material.
            let _ = sources.store.invalidate(reference);
            Err(EvidenceRefusal::Unavailable)
        }
    }
}

mod scoped_trust;
pub(crate) use scoped_trust::ScopedTrust;
mod linux_scoped_trust;
pub(crate) use linux_scoped_trust::LinuxScopedTrust;
mod sources;
pub(crate) use sources::Sources;
mod scoped_evidence_port;
pub(crate) use scoped_evidence_port::ScopedEvidencePort;
