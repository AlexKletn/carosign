use super::EvidenceCheck;

use carosign_core::assurance::AssuranceAssessment;
use carosign_evidence::material::EvidenceMaterialRef;
use carosign_evidence::requirements::EvidenceRequirements;
use carosign_evidence::verifier::Verdict;
use carosign_runtime::ports::{EvidenceContext, EvidencePort, EvidenceRefusal};

/// Displaying the refusal of proof of port failure.
///
/// A mismatch is an operation outcome. Proof validation errors are reported separately.
/// It's just rejection.
pub fn refusal_of(verdict: &Verdict) -> Option<EvidenceRefusal> {
    match verdict {
        Verdict::Accepted(_) => None,
        Verdict::Rejected(error) => Some(match error {
            carosign_evidence::material::EvidenceError::UnsupportedFormat => {
                EvidenceRefusal::Unsupported
            }
            carosign_evidence::material::EvidenceError::WrongRecipient => {
                EvidenceRefusal::WrongRecipient
            }
            carosign_evidence::material::EvidenceError::Expired => EvidenceRefusal::Expired,
            carosign_evidence::material::EvidenceError::VerifierUnavailable => {
                EvidenceRefusal::Unavailable
            }
            _ => EvidenceRefusal::Unverified,
        }),
    }
}

/// The proof port adapter.
pub struct EvidenceBridge<V: EvidenceCheck> {
    checker: V,
    store: Option<std::sync::Arc<carosign_evidence::store::EvidenceStore>>,
}

impl<V: EvidenceCheck> core::fmt::Debug for EvidenceBridge<V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("EvidenceBridge(..)")
    }
}

impl<V: EvidenceCheck> EvidenceBridge<V> {
    /// Adapter over the checker.
    pub fn new(checker: V) -> Self {
        Self {
            checker,
            store: None,
        }
    }

    /// Links the review to the owner of the material; the owner does not confirm the origin
    /// and does not allow a positive assessment.
    pub fn with_store(
        checker: V,
        store: std::sync::Arc<carosign_evidence::store::EvidenceStore>,
    ) -> Self {
        Self {
            checker,
            store: Some(store),
        }
    }
}

impl<V: EvidenceCheck> EvidencePort for EvidenceBridge<V> {
    fn evaluate<'a>(
        &'a self,
        material: EvidenceMaterialRef,
        context: &'a EvidenceContext,
        requirements: &'a EvidenceRequirements,
    ) -> carosign_runtime::BoxFuture<'a, Result<AssuranceAssessment, EvidenceRefusal>> {
        if material.operation() != *context.operation.as_bytes()
            || context.enrollment.authenticator != context.authenticator.id
            || context.purpose != requirements.purpose
            || requirements.validate().is_err()
            || context
                .permitted
                .bindings()
                .iter()
                .any(|binding| binding.enrollment.authenticator != context.authenticator.id)
            || !context
                .permitted
                .bindings()
                .iter()
                .any(|binding| binding.enrollment == context.enrollment)
        {
            return Box::pin(async { Err(EvidenceRefusal::Unverified) });
        }
        let verdict = self.checker.check(material, context, requirements);
        let result = match verdict {
            // The context alone does not confirm the origin of the registration and the
            // material; this adapter cannot give a positive rating.
            Verdict::Accepted(_) => Err(EvidenceRefusal::Unverified),
            Verdict::Rejected(error) => {
                Err(refusal_of(&Verdict::Rejected(error)).unwrap_or(EvidenceRefusal::Unverified))
            }
        };
        Box::pin(async move { result })
    }

    fn invalidate(
        &self,
        material: EvidenceMaterialRef,
    ) -> carosign_runtime::BoxFuture<'_, Result<(), EvidenceRefusal>> {
        let result = self
            .store
            .as_ref()
            .ok_or(EvidenceRefusal::Unavailable)
            .and_then(|store| {
                store
                    .invalidate(material)
                    .map_err(|_| EvidenceRefusal::Unavailable)
            });
        Box::pin(async move { result })
    }
}
