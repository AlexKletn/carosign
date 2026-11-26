use super::super::CleanupScope;
use super::Stage;

/// Surveillance obtained by a trusted hardware adapter.
/// Value cannot be intentionally generated from an worker’s response with a secure code.
pub struct Observation {
    pub(super) scope: CleanupScope,
    pub(super) reached: Stage,
}

impl Observation {
    /// # Safety
    /// The trusted adapter directly checked the specified condition for the entire issue
    /// `scope`A sign from the provider's process, its completion or the matching of response
    /// fields does not in themselves prove this.
    /// The status achieved shall be maintained until the verification and release are
    /// completed.
    pub unsafe fn from_trusted_hardware(scope: CleanupScope, reached: Stage) -> Self {
        Self { scope, reached }
    }
}
