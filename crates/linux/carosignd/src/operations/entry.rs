use super::{
    ConnectionHandle, OwnedOperation, RetainedOwnerVerification, VerificationBindingStage,
    VerificationDispatchStage,
};
use std::sync::Mutex;

pub(super) struct Entry {
    pub(super) is_mutation: bool,
    pub(super) cancellation: Option<std::sync::Weak<crate::cancel_reader::CancellationTarget>>,
    pub(super) subject: carosign_core::identity::SubjectRef,
    pub(super) owner: [u8; 16],
    pub(super) owner_connection: Option<ConnectionHandle>,
    pub(super) operation: Option<OwnedOperation>,
    pub(super) deadline: carosign_core::operation::context::BootDeadline,
    pub(super) pending_control: Vec<carosign_runtime::OperationMsg>,
    pub(super) events: std::sync::Arc<Mutex<carosign_runtime::operation::hub::EventHub>>,
    pub(super) delivery_invalidated: bool,
    pub(super) terminal: bool,
    #[allow(dead_code)]
    pub(super) verification_dispatch: VerificationDispatchStage,
    #[allow(dead_code)]
    pub(super) verification_dispatch_step: Option<carosign_runtime::operation::actor::Step>,
    pub(super) verification_binding: VerificationBindingStage,
    pub(super) retained_delivery: Option<RetainedOwnerVerification>,
    pub(super) positive_reservation: Option<carosign_runtime::operation::hub::TerminalReservation>,
    pub(super) matched_evidence_step: Option<(
        carosign_runtime::operation::actor::Step,
        carosign_evidence::material::EvidenceMaterialRef,
    )>,
    pub(super) matched_enrollment: Option<carosign_core::enrollment::EnrollmentRef>,
    pub(super) evidence_evaluation_step: Option<(
        carosign_engine::PlannedEffect,
        carosign_runtime::operation::actor::Step,
    )>,
    pub(super) owner_revalidation_step: Option<(
        carosign_engine::PlannedEffect,
        carosign_runtime::operation::actor::Step,
    )>,
    pub(super) evidence_destroy_step: Option<(
        carosign_engine::PlannedEffect,
        carosign_runtime::operation::actor::Step,
        carosign_evidence::material::EvidenceMaterialRef,
    )>,
    pub(super) mutation_binding_step: Option<carosign_runtime::operation::actor::Step>,
}
