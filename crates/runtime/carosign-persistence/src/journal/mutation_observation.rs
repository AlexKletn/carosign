use carosign_core::ids::MutationId;

use crate::model::MutationPhase;

/// A provider's surveillance that needs to be recorded.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MutationObservation {
    pub mutation: MutationId,
    /// ISP response: `Completed`, `NoChange`, `MayHaveOccurred`.
    pub phase: MutationPhase,
    /// Recovery link, if the provider has given it.
    pub recovery_ref: Option<carosign_core::enrollment::OpaqueProviderRef>,
}
