use crate::reconcile::ReconciliationVerdict;

/// What happened to the change (entry after addressing check)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MutationInput {
    /// The authority to manage the factor is confirmed.
    ManageGrantValid,
    /// Intent is on the disc.
    IntentDurable,
    /// The intention did not fall - nothing went on the hardware.
    IntentFailed,
    /// Resources issued.
    LeaseGranted,
    /// Resources not issued.
    LeaseDenied,
    /// The powers are still in place.
    GrantValid,
    GrantRevoked,
    /// `DispatchArmed` Down on the disk.
    DispatchArmedDurable,
    /// `DispatchArmed` Don't lie down, you can't send.
    DispatchArmedFailed,
    /// The worker accepted the changing command, its outcome is not yet known.
    DispatchAccepted,
    /// The provider has completed the change.
    ProviderCompleted,
    /// The provider claims that nothing has changed.
    ProviderNoChange,
    TrustedEnrollmentRollback,
    /// The provider claims that it could have happened.
    ProviderMayHaveOccurred,
    /// The provider said no.
    ProviderFailed,
    /// The shipment did not reach the metal (confirmed).
    NotSent,
    /// Whether the shipment reached is unknown.
    OutcomeUnknown,
    /// The worker's missing.
    WorkerLost,
    /// Outcome of independent verification.
    Reconciled(ReconciliationVerdict),
    /// The result of a trusted provider in a live session.
    TrustedProviderCompleted,
    /// The vault is not available: uncertainty is not recorded.
    StorageUnavailable,
}
