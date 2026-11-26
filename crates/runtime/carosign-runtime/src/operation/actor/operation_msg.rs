use crate::operation::mailbox::Priority;
use carosign_engine::{DomainInput, EffectReceipt};

/// That came in the actor's mail.
///
/// ⚠️ Each message is a verified observation: the operation number, the era and the worker
/// are checked before it is here.
#[derive(Clone, PartialEq, Debug)]
pub enum OperationMsg {
    /// Engine input (acceptance, effect confirmation, provider supervision).
    Domain(DomainInput),
    /// A cancellation request from the owner came in.
    CancelRequested,
    /// The owner passed out.
    OwnerDetached,
    GrantRevoked,
    /// Deadline's here.
    DeadlineElapsed,
}

impl OperationMsg {
    /// Which queue it belongs to.
    ///
    /// ⚠️ Management is what extinguishes success: recall, deadline, cancellation, disabling
    /// the owner.
    pub fn priority(&self) -> Priority {
        match self {
            Self::CancelRequested
            | Self::OwnerDetached
            | Self::GrantRevoked
            | Self::DeadlineElapsed => Priority::Control,
            Self::Domain(
                DomainInput::OwnerRevoked
                | DomainInput::DeadlineElapsed
                | DomainInput::OwnerDetached
                | DomainInput::CancelRequested
                | DomainInput::WorkerLost
                | DomainInput::DeviceEpochChanged(_)
                | DomainInput::StorageUnavailable
                | DomainInput::Reconciliation(_),
            ) => Priority::Control,
            Self::Domain(DomainInput::Receipt {
                receipt: EffectReceipt::MetadataFailed,
                ..
            }) => Priority::Control,
            Self::Domain(_) => Priority::Normal,
        }
    }

    /// Transfers the engine to the entrance.
    pub(super) fn into_domain(self) -> DomainInput {
        match self {
            Self::Domain(input) => input,
            Self::CancelRequested => DomainInput::CancelRequested,
            Self::OwnerDetached => DomainInput::OwnerDetached,
            Self::GrantRevoked => DomainInput::OwnerRevoked,
            Self::DeadlineElapsed => DomainInput::DeadlineElapsed,
        }
    }
}
