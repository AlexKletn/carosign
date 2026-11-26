use carosign_engine::OperationPhase as Phase;
use carosign_protocol::carosign::client::v1::OperationPhase as PublicPhase;

pub(crate) fn public_phase(phase: Phase) -> PublicPhase {
    match phase {
        Phase::New | Phase::WaitingLease => PublicPhase::OpQueued,
        Phase::ReadyToDispatch
        | Phase::Running
        | Phase::EvaluatingEvidence
        | Phase::Preparing
        | Phase::Arming
        | Phase::InFlight
        | Phase::ValidatingChange
        | Phase::Committing => PublicPhase::OpRunning,
        Phase::Reconciling => PublicPhase::OpNeedsReconciliation,
        Phase::Cancelling => PublicPhase::OpCancelling,
        Phase::Finalizing => PublicPhase::OpFinalizing,
        Phase::Terminal => PublicPhase::OpTerminal,
    }
}
