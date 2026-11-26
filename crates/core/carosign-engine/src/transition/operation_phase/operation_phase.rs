use super::PhaseFamily;
use core::fmt;

/// The operation phase is common to all machines.
///
/// ⚠️ The phase answers the question of “where are we” rather than “what have we done?”
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationPhase {
    /// Accepted, but not yet admitted.
    New,
    /// Waiting for resources.
    WaitingLease,
    /// Resources are available and powers are rechecked – you can send them.
    ReadyToDispatch,
    /// Command's out, awaiting observation.
    Running,
    /// After the match, we check the proof.
    EvaluatingEvidence,
    /// Change prepared: nothing has been done on the hardware.
    Preparing,
    /// The decision to “send” is recorded on the disk.
    Arming,
    /// Changing command sent.
    InFlight,
    /// The provider has a response, we check the actual changes.
    ValidatingChange,
    /// Enter the registry and the log with one transaction.
    Committing,
    /// The change could have happened; an independent reconciliation is needed.
    Reconciling,
    /// Sending is cancelled; the result is suppressed.
    Cancelling,
    /// Resource cleanup and delivery.
    Finalizing,
    Terminal,
}

impl fmt::Debug for OperationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::New => "New",
            Self::WaitingLease => "WaitingLease",
            Self::ReadyToDispatch => "ReadyToDispatch",
            Self::Running => "Running",
            Self::EvaluatingEvidence => "EvaluatingEvidence",
            Self::Preparing => "Preparing",
            Self::Arming => "Arming",
            Self::InFlight => "InFlight",
            Self::ValidatingChange => "ValidatingChange",
            Self::Committing => "Committing",
            Self::Reconciling => "Reconciling",
            Self::Cancelling => "Cancelling",
            Self::Finalizing => "Finalizing",
            Self::Terminal => "Terminal",
        })
    }
}

impl OperationPhase {
    /// It doesn't change anymore.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Terminal)
    }

    /// `DispatchArmed` It's already on the disk.
    ///
    /// ⚠️ Separate from "sent": between them lies a dangerous window.
    pub fn is_armed(self) -> bool {
        matches!(
            self,
            Self::Arming
                | Self::InFlight
                | Self::ValidatingChange
                | Self::Committing
                | Self::Reconciling
        )
    }

    /// The command could have gone on the hardware.
    pub fn may_have_reached_hardware(self) -> bool {
        matches!(
            self,
            Self::InFlight | Self::ValidatingChange | Self::Committing | Self::Reconciling
        )
    }
}

impl OperationPhase {
    /// Which machine is the phase?
    pub fn family(self) -> PhaseFamily {
        match self {
            // ⚠️ `WaitingLease` and `ReadyToDispatch` Common to both machines: both take
            // resources before sending anything.
            Self::WaitingLease | Self::ReadyToDispatch => PhaseFamily::Shared,
            Self::Running | Self::EvaluatingEvidence => PhaseFamily::Verification,
            Self::Preparing
            | Self::Arming
            | Self::InFlight
            | Self::ValidatingChange
            | Self::Committing
            | Self::Reconciling => PhaseFamily::Mutation,
            // `New`, `Cancelling`, `Finalizing`, `Terminal` They are also common, but they do
            // not have their own car-owner; we will classify them as common.
            Self::New | Self::Cancelling | Self::Finalizing | Self::Terminal => PhaseFamily::Shared,
        }
    }
}
