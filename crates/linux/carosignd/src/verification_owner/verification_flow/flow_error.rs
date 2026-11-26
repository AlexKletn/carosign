use super::{AdvanceError, MatchedEvidenceError, StartError};

#[derive(Debug)]
pub(crate) enum FlowError {
    Admission(crate::operations::IdempotentAdmission),
    Watch(crate::control_watch::StartError),
    ContextMismatch,
    Operation(crate::operations::ObservationFailure),
    Preparation(crate::run::RunOutcome),
    Start(StartError),
    Handoff(crate::effect_owner::EffectOwnerError),
    Drive(crate::effect_owner::DriveError),
    Advance(AdvanceError),
    Delivery(crate::operations::OwnerDeliveryFailure),
    Send(crate::operations::OwnerSendError),
    MissingTransport,
    InvalidStage,
    EvidenceId(carosign_core::ids::IdUnavailable),
    Evidence(MatchedEvidenceError),
    ScopedEvidence(carosign_runtime::ports::EvidenceRefusal),
    Authority(carosign_runtime::ports::AuthorityRefusal),
}
