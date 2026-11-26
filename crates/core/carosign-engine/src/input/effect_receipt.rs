use super::EffectSpec;

/// Confirmation of the effect, returned from the time of work.
///
/// ⚠️ It's observation. Confirmation "sent" doesn't mean "completed."
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectReceipt {
    /// Intent is on the disc.
    IntentDurable,
    /// The intention didn't go away.
    IntentFailed,
    /// `DispatchArmed` Down on the disk.
    DispatchArmedDurable,
    /// `DispatchArmed` Don't lie down, you can't send.
    DispatchArmedFailed,
    /// Resources issued.
    LeaseGranted,
    /// Resources not issued.
    LeaseDenied,
    /// The powers are still in place.
    GrantValid,
    GrantRevoked,
    /// It is accepted (doesn't mean that the hardware has been made).
    SendAccepted,
    /// Verification: Confirmed that command **not* passed.
    ///
    /// ⚠️ This is a launch error, not a matching error (B-02); there is no stable barrier to
    /// verification. `DispatchArmed`.
    DispatchNotSent,
    /// Check: It is not known whether the command has been able to start the execution.
    DispatchOutcomeUnknown,
    /// The changing command is accepted by the worker, but its outcome is still unknown.
    MutationAccepted,
    /// Change: the changing command **not* is transmitted, `DispatchArmed` on the disc.
    ///
    /// ⚠️ After the arm, this does not prove the initial state of the device: the operation
    /// remains on reconciliation, re-sending is prohibited.
    MutationNotSent,
    /// Change: It is not known if the changing command has reached.
    MutationOutcomeUnknown,
    /// Proof accepted.
    EvidenceAccepted,
    /// The evidence is denied.
    EvidenceRejected,
    /// The owner's on.
    OwnerValid,
    /// Owner recalled/disconnected.
    OwnerRevoked,
    /// Metadata's on the disc.
    MetadataDurable,
    /// The negative result of the test was recorded only in the current launch.
    VerificationTerminalRecorded,
    /// The desired state is permanently fixed.
    MetadataCommitted,
    /// A steady state has been recorded.
    MetadataNoChange,
    /// Methadata didn't go down.
    MetadataFailed,
    /// The actual condition is confirmed (with reference and origin).
    StateConfirmed(crate::reconcile::VerifiedState),
    /// The condition can't be confirmed.
    StateUnknown,
    /// A trusted Linux component confirmed the result in a live session.
    /// It is not used to restore an unknown outcome.
    TrustedProviderCompleted,
    /// Cleaning confirmed.
    CleanupConfirmed,
    /// The status of resources is unclear.
    ResourceUncertain,
    /// The exodus has been delivered to the owner.
    OwnerDeliveryRecorded,
    /// Owner blacked out before delivery.
    OwnerDisconnected,
    /// Cancellation sent.
    CancelDispatched,
    /// Cancellation is not supported.
    CancelUnsupported,
    /// The contractor and the broker are complete; the condition of the device still
    /// requires cleaning.
    CancelWorkerStopped,
    /// The result is withdrawn by the owner of the transaction and can no longer be issued.
    ResultSuppressed,
    /// Motion posted.
    ProgressPublished,
    EvidenceDestroyed,
    /// Recording "required reconciliation" went to the disk.
    ReconciliationPersisted,
    /// The area is fenced.
    ScopeQuarantined,
}

impl EffectReceipt {
    /// Whether the confirmation corresponds to the type of effect issued.
    pub fn confirms(self, effect: EffectSpec) -> bool {
        if self == Self::MetadataFailed {
            return matches!(
                effect,
                EffectSpec::CommitDesiredRegistryAndJournal
                    | EffectSpec::CommitNoChangeAndJournal
                    | EffectSpec::RecordTerminal
            );
        }
        let expected = match self {
            Self::IntentDurable | Self::IntentFailed => EffectSpec::PersistIntent,
            Self::DispatchArmedDurable | Self::DispatchArmedFailed => {
                EffectSpec::PersistDispatchArmed
            }
            Self::LeaseGranted | Self::LeaseDenied => EffectSpec::AcquireResources,
            Self::GrantValid | Self::GrantRevoked => EffectSpec::RevalidateGrant,
            Self::SendAccepted | Self::DispatchNotSent | Self::DispatchOutcomeUnknown => {
                EffectSpec::SendVerify
            }
            Self::MutationAccepted | Self::MutationNotSent | Self::MutationOutcomeUnknown => {
                EffectSpec::SendMutatingCommand
            }
            Self::EvidenceAccepted | Self::EvidenceRejected | Self::EvidenceDestroyed => {
                if matches!(self, Self::EvidenceDestroyed) {
                    EffectSpec::DestroyEvidence
                } else {
                    EffectSpec::VerifyEvidence
                }
            }
            Self::OwnerValid | Self::OwnerRevoked => EffectSpec::RevalidateOwner,
            Self::MetadataDurable | Self::VerificationTerminalRecorded => {
                EffectSpec::RecordTerminal
            }
            Self::MetadataFailed => unreachable!(),
            Self::MetadataCommitted => EffectSpec::CommitDesiredRegistryAndJournal,
            Self::MetadataNoChange => EffectSpec::CommitNoChangeAndJournal,
            Self::StateConfirmed(_) | Self::StateUnknown | Self::TrustedProviderCompleted => {
                EffectSpec::QueryProviderState
            }
            Self::CleanupConfirmed | Self::ResourceUncertain => EffectSpec::ReleaseResources,
            Self::OwnerDeliveryRecorded | Self::OwnerDisconnected => EffectSpec::DeliverOutcome,
            Self::CancelDispatched | Self::CancelUnsupported | Self::CancelWorkerStopped => {
                EffectSpec::RequestCancel
            }
            Self::ResultSuppressed => EffectSpec::SuppressResult,
            Self::ProgressPublished => EffectSpec::PublishProgress,
            Self::ReconciliationPersisted => EffectSpec::PersistNeedsReconciliation,
            Self::ScopeQuarantined => EffectSpec::QuarantineScope,
        };
        expected == effect
    }

    /// Is this a good outcome for its effect?
    pub fn is_success(self) -> bool {
        matches!(
            self,
            Self::IntentDurable
                | Self::DispatchArmedDurable
                | Self::LeaseGranted
                | Self::GrantValid
                | Self::SendAccepted
                | Self::MutationAccepted
                | Self::EvidenceAccepted
                | Self::OwnerValid
                | Self::MetadataDurable
                | Self::VerificationTerminalRecorded
                | Self::MetadataCommitted
                | Self::MetadataNoChange
                | Self::StateConfirmed(_)
                | Self::CleanupConfirmed
                | Self::OwnerDeliveryRecorded
                | Self::CancelDispatched
                | Self::CancelWorkerStopped
                | Self::ResultSuppressed
                | Self::ProgressPublished
                | Self::EvidenceDestroyed
                | Self::ReconciliationPersisted
                | Self::ScopeQuarantined
        )
    }
}
