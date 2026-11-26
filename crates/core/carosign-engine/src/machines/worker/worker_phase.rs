use super::WorkerEvent;
use core::fmt;

/// Worker phase.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkerPhase {
    /// Process running, wait. `WorkerHello`.
    Starting,
    /// We're negotiating a version.
    Negotiating,
    /// We give out resources and wait. `AuthenticatorReady`.
    Binding,
    /// Ready for commands.
    Ready,
    /// Let's go.
    Draining,
    /// Rejection: Session closed.
    Faulted,
}

impl fmt::Debug for WorkerPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Starting => "Starting",
            Self::Negotiating => "Negotiating",
            Self::Binding => "Binding",
            Self::Ready => "Ready",
            Self::Draining => "Draining",
            Self::Faulted => "Faulted",
        })
    }
}

/// The next phase of the worker.
///
/// ⚠️ Jumping through a phase is a failure, not a guess. You can't wait for a sign that's
/// already there: the event must be what you expected.
pub fn next_phase(phase: WorkerPhase, event: WorkerEvent) -> Option<WorkerPhase> {
    use WorkerEvent as W;
    match (phase, event) {
        (_, W::Lost | W::ProtocolFault) => Some(WorkerPhase::Faulted),
        (WorkerPhase::Starting, W::Hello) => Some(WorkerPhase::Negotiating),
        (WorkerPhase::Negotiating, W::Initialized) => Some(WorkerPhase::Binding),
        (WorkerPhase::Binding, W::AuthenticatorReady) => Some(WorkerPhase::Ready),
        (WorkerPhase::Ready, W::Quiesce) => Some(WorkerPhase::Draining),
        _ => None,
    }
}

/// Phase waiting time, seconds (Document 13).
pub fn phase_timeout_s(phase: WorkerPhase) -> Option<u8> {
    match phase {
        WorkerPhase::Starting | WorkerPhase::Negotiating => {
            Some(carosign_core::error::limits::PROVIDER_HANDSHAKE_TIMEOUT_S)
        }
        WorkerPhase::Binding => Some(carosign_core::error::limits::PROVIDER_BIND_TIMEOUT_S),
        _ => None,
    }
}

#[path = "worker_phase.tests.rs"]
#[cfg(test)]
mod tests;
