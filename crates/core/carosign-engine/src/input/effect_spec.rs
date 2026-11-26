use core::fmt;

/// The effect that the engine asks for.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectSpec {
    /// Write down the intention of the change (before any work with hardware).
    PersistIntent,
    /// Persist `DispatchArmed` before sending the mutation command.
    PersistDispatchArmed,
    /// Take resources completely and atomically.
    AcquireResources,
    /// Re-check the authority at the border.
    RevalidateGrant,
    /// Send a vetting command.
    SendVerify,
    /// Send a changing command - once, without repeating.
    SendMutatingCommand,
    /// Verify the evidence independently.
    VerifyEvidence,
    /// Check the owner before delivery.
    RevalidateOwner,
    DestroyEvidence,
    /// To fix the desired state of the registry and the journal.
    CommitDesiredRegistryAndJournal,
    /// Record the verified NoChange without changing the registry.
    CommitNoChangeAndJournal,
    /// Check the actual state independently.
    QueryProviderState,
    /// Re-read the stable journal and registry after ambiguous fixation.
    RecoverMetadataCommit,
    /// Write down "need a reconciliation."
    PersistNeedsReconciliation,
    /// Protect the area from new changes.
    QuarantineScope,
    /// Ask for cancellations separately.
    RequestCancel,
    SuppressResult,
    /// Free up resources.
    ReleaseResources,
    /// Record the terminal state.
    RecordTerminal,
    /// Deliver the outcome (once).
    DeliverOutcome,
    /// Publish the move (can be glued together).
    PublishProgress,
}

impl EffectSpec {
    /// Name for the magazine.
    pub fn name(self) -> &'static str {
        match self {
            Self::PersistIntent => "PersistIntent",
            Self::PersistDispatchArmed => "PersistDispatchArmed",
            Self::AcquireResources => "AcquireResources",
            Self::RevalidateGrant => "RevalidateGrant",
            Self::SendVerify => "SendVerify",
            Self::SendMutatingCommand => "SendMutatingCommand",
            Self::VerifyEvidence => "VerifyEvidence",
            Self::RevalidateOwner => "RevalidateOwner",
            Self::DestroyEvidence => "DestroyEvidence",
            Self::CommitDesiredRegistryAndJournal => "CommitDesiredRegistryAndJournal",
            Self::CommitNoChangeAndJournal => "CommitNoChangeAndJournal",
            Self::QueryProviderState => "QueryProviderState",
            Self::RecoverMetadataCommit => "RecoverMetadataCommit",
            Self::PersistNeedsReconciliation => "PersistNeedsReconciliation",
            Self::QuarantineScope => "QuarantineScope",
            Self::RequestCancel => "RequestCancel",
            Self::SuppressResult => "SuppressResult",
            Self::ReleaseResources => "ReleaseResources",
            Self::RecordTerminal => "RecordTerminal",
            Self::DeliverOutcome => "DeliverOutcome",
            Self::PublishProgress => "PublishProgress",
        }
    }

    /// Can the effect be repeated without harm?
    ///
    /// ⚠️ Only explicitly listed. The changing shipment does not repeat.
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::PublishProgress | Self::DestroyEvidence)
    }

    /// Does it change the stable state on the hardware?
    pub fn mutates_hardware(self) -> bool {
        matches!(self, Self::SendMutatingCommand)
    }

    /// This effect concerns durable storage.
    pub fn is_durable_write(self) -> bool {
        matches!(
            self,
            Self::PersistIntent
                | Self::PersistDispatchArmed
                | Self::PersistNeedsReconciliation
                | Self::CommitDesiredRegistryAndJournal
                | Self::CommitNoChangeAndJournal
                | Self::RecordTerminal
                | Self::QuarantineScope
        )
    }
}

impl fmt::Debug for EffectSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
