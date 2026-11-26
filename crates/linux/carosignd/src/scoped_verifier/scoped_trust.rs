use super::{EvidenceRefusal, ScopedAssertionContext, ScopedProviderAssertion};

/// An independent system source must validate the worker, the right to programmatic
/// evaluation, the recipient, and the current generation of policies; an internal interface
/// does not automatically make arbitrary implementation trusted.
pub(crate) trait ScopedTrust: Send + Sync {
    fn confirm(
        &self,
        assertion: &ScopedProviderAssertion,
        context: &ScopedAssertionContext,
    ) -> Result<(), EvidenceRefusal>;
}
