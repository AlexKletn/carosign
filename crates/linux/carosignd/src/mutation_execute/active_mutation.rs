use carosign_core::ids::IdSource;
use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, transition::EffectKey};
use carosign_persistence::{
    mutation_records::ObservedMutation as JournalObservation, store::MetadataStore,
};
use carosign_protocol::carosign::provider::v1::{
    EnrollmentTransactionAction, EnrollmentTransactionRequest, EnrollmentTransactionState,
    ProviderFrame, provider_frame::Payload,
};
use prost::Message;
use std::time::{Duration, Instant};

use carosign_core::ids::OperationId;
use carosign_engine::PlannedEffect;
use carosign_runtime::ports::PolicyAuthority;
use std::sync::Arc;

use super::{
    EnrollmentAuthorizationGuard, MutationReception, MutationRequest, MutationStartError,
    ObservedMutation, PreparedMutation, actor_advance, actor_effect, release_mutation_resources,
};

/// Owns the provider channel and processes through result validation and cleanup.
pub(crate) struct ActiveMutation {
    pub(crate) operation: OperationId,
    pub(crate) mutation: carosign_core::ids::MutationId,
    pub(crate) command: carosign_core::ids::CommandId,
    pub(crate) worker: carosign_core::ids::WorkerInstanceId,
    pub(crate) epoch: carosign_core::device::DeviceEpoch,
    pub(crate) effect: PlannedEffect,
    pub(crate) cleanup: Arc<crate::resources::SupervisorCleanup>,
    pub(crate) channel: carosign_linux_ipc::provider::tunnel::TunnelProviderChannel,
    pub(crate) control: Option<Arc<crate::mutation_control::MutationControl>>,
    pub(super) rollback_enabled: bool,
    pub(super) authorization: Option<EnrollmentAuthorizationGuard>,
    pub(super) format_id: String,
    pub(super) format_version: u32,
}

impl ActiveMutation {
    pub(crate) async fn receive<S: carosign_persistence::store::MetadataStore>(
        self,
        prepared: &PreparedMutation,
        store: &S,
        policy: &crate::linux_policy::LinuxPolicy,
        registry: &crate::operations::OperationRegistry,
        resources: &crate::resources::LocalResourceOwner,
        mut deliver_progress: impl FnMut(
            carosign_runtime::operation::events::Event,
        ) -> Result<(), MutationStartError>,
    ) -> Result<MutationReception, MutationStartError> {
        use carosign_protocol::carosign::provider::v1::{MutationStatus, provider_frame::Payload};
        use prost::Message;
        let mut acknowledged = false;
        let mut progress_session = carosign_runtime::provider::session::Session {
            worker: Some(self.worker),
            device_epoch: Some(self.epoch),
            operation: Some(self.operation),
            ..Default::default()
        };
        let operation_deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let idle_deadline = operation_deadline.min(Instant::now() + Duration::from_secs(30));
            loop {
                if self.rollback_enabled
                    && self
                        .control
                        .as_ref()
                        .is_some_and(|control| control.rollback_requested())
                {
                    return self
                        .enrollment_rollback(store, resources, registry, None)
                        .await;
                }
                let remaining = idle_deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    if self.rollback_enabled && acknowledged {
                        return self
                            .enrollment_rollback(store, resources, registry, None)
                            .await;
                    }
                    return Err(MutationStartError::Provider);
                }
                if self
                    .channel
                    .has_event(remaining.min(Duration::from_millis(20)))
                    .map_err(|_| MutationStartError::Provider)?
                {
                    break;
                }
            }
            let frame = self
                .channel
                .receive()
                .map_err(|_| MutationStartError::Provider)?;
            if frame.worker_instance_id != self.worker.as_bytes()
                || frame.operation_id != self.operation.as_bytes()
                || frame.command_id != self.command.as_bytes()
                || frame.device_epoch != self.epoch.value()
            {
                return Err(MutationStartError::Context);
            }
            match &frame.payload {
                Some(Payload::CommandAck(ack))
                    if !acknowledged && ack.accepted && ack.rejection.is_none() =>
                {
                    actor_advance(
                        registry,
                        self.operation,
                        DomainInput::Receipt {
                            effect: self.effect,
                            receipt: EffectReceipt::MutationAccepted,
                        },
                    )?;
                    acknowledged = true;
                    progress_session.accepted = Some(self.command);
                }
                Some(Payload::ProviderProgress(progress)) if acknowledged => {
                    if (progress.feedback.is_some() || progress.capture_state.is_some())
                        && matches!(prepared.request, MutationRequest::Remove { .. })
                    {
                        return Err(MutationStartError::Provider);
                    }
                    let event = carosign_provider_sdk::events::parse(&frame)
                        .map_err(|_| MutationStartError::Provider)?;
                    let observation = progress_session
                        .accept_progress_event(event)
                        .map_err(|_| MutationStartError::Provider)?;
                    if (progress.feedback.is_some() || progress.capture_state.is_some())
                        && let Some(event) = registry
                            .publish_provider_progress(observation)
                            .map_err(|_| MutationStartError::State)?
                    {
                        policy
                            .revalidate_caller(
                                &prepared.caller,
                                &prepared.grant,
                                carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
                            )
                            .await
                            .map_err(|_| MutationStartError::State)?;
                        deliver_progress(event)?;
                    }
                }
                Some(Payload::MutationOutcome(outcome))
                    if acknowledged
                        && self.rollback_enabled
                        && outcome.mutation_id == self.mutation.as_bytes()
                        && outcome.status == MutationStatus::MutationMayHaveOccurred as i32
                        && outcome.enrollment.is_none()
                        && outcome.provider_opaque_ref.is_empty()
                        && outcome.recovery_ref.is_empty() =>
                {
                    use carosign_protocol::carosign::common::v1::ErrorCode;
                    let reason = outcome
                        .error
                        .as_ref()
                        .map(|error| error.code())
                        .filter(|code| {
                            matches!(
                                code,
                                ErrorCode::ErrorCaptureStorageFull
                                    | ErrorCode::ErrorDuplicateSample
                                    | ErrorCode::ErrorDeviceRemoved
                            )
                        });
                    return self
                        .enrollment_rollback(store, resources, registry, reason)
                        .await;
                }
                Some(Payload::MutationOutcome(outcome)) if acknowledged => {
                    if outcome.mutation_id != self.mutation.as_bytes()
                        || outcome.status != MutationStatus::MutationProviderCompleted as i32
                        || outcome.error.is_some()
                    {
                        return Err(MutationStartError::Provider);
                    }
                    let target = match prepared
                        .plan
                        .to_core(carosign_persistence::mutation_plan::Scope {
                            subject: prepared.subject,
                            authenticator: prepared.authenticator,
                        })
                        .map_err(|_| MutationStartError::Context)?
                        .transition()
                    {
                        carosign_core::operation::plan::PlanTransition::Create { planned_new }
                        | carosign_core::operation::plan::PlanTransition::Replace {
                            planned_new,
                            ..
                        } => planned_new,
                        carosign_core::operation::plan::PlanTransition::Remove { old } => old,
                    };
                    let echo = outcome
                        .enrollment
                        .as_ref()
                        .ok_or(MutationStartError::Context)?;
                    if echo.enrollment_id != target.id.as_bytes()
                        || echo.authenticator_id != target.authenticator.as_bytes()
                        || echo.generation != target.generation.value()
                        || matches!(prepared.request, MutationRequest::Remove { .. })
                            && !outcome.provider_opaque_ref.is_empty()
                    {
                        return Err(MutationStartError::Context);
                    }
                    policy
                        .revalidate_caller(
                            &prepared.caller,
                            &prepared.grant,
                            carosign_core::authorization::AuthorizationBoundary::BeforeDispatch,
                        )
                        .await
                        .map_err(|_| MutationStartError::Authority)?;
                    let digest =
                        carosign_persistence::mutation_plan::sha256(&frame.encode_to_vec());
                    let observation = carosign_persistence::mutation_records::ObservedMutation {
                        mutation: self.mutation,
                        operation: self.operation,
                        command: self.command,
                        worker: self.worker,
                        epoch: self.epoch,
                        effect: carosign_engine::transition::EffectKey {
                            operation: self.operation,
                            transition_index: self.effect.transition,
                            position: carosign_engine::transition::EffectPosition::new(
                                self.effect.position,
                            ),
                        },
                        observation_digest: digest,
                        recovery_ref: if outcome.recovery_ref.is_empty() {
                            None
                        } else {
                            Some(
                                carosign_core::enrollment::OpaqueProviderRef::new(
                                    outcome.recovery_ref.clone(),
                                )
                                .map_err(|_| MutationStartError::Context)?,
                            )
                        },
                    };
                    let mut tx = store
                        .begin_write()
                        .map_err(|_| MutationStartError::Storage)?;
                    carosign_persistence::mutation_store::stage_observation_v4(
                        &mut *tx,
                        &observation,
                    )
                    .map_err(|_| MutationStartError::Storage)?;
                    tx.commit_durable()
                        .map_err(|_| MutationStartError::Storage)?;
                    let query_effect = actor_effect(
                        registry,
                        self.operation,
                        Some(DomainInput::ProviderCompleted { echoed: target }),
                        EffectSpec::QueryProviderState,
                    )?;
                    let provider_ref = outcome.provider_opaque_ref.clone();
                    return Ok(MutationReception::Observed(ObservedMutation {
                        active: self,
                        query_effect,
                        digest,
                        provider_ref,
                    }));
                }
                _ => return Err(MutationStartError::Provider),
            }
        }
    }
}

impl ActiveMutation {
    pub(super) async fn enrollment_rollback(
        mut self,
        store: &impl MetadataStore,
        resources: &crate::resources::LocalResourceOwner,
        registry: &crate::operations::OperationRegistry,
        reason: Option<carosign_protocol::carosign::common::v1::ErrorCode>,
    ) -> Result<MutationReception, MutationStartError> {
        let digest = self.request_rollback()?;
        let commit = actor_effect(
            registry,
            self.operation,
            Some(DomainInput::TrustedEnrollmentRollback),
            EffectSpec::CommitNoChangeAndJournal,
        )?;
        let observation = JournalObservation {
            mutation: self.mutation,
            operation: self.operation,
            command: self.command,
            worker: self.worker,
            epoch: self.epoch,
            effect: EffectKey {
                operation: self.operation,
                transition_index: self.effect.transition,
                position: carosign_engine::transition::EffectPosition::new(self.effect.position),
            },
            observation_digest: digest,
            recovery_ref: None,
        };
        carosign_persistence::mutation_store::commit_enrollment_rollback_v4(store, &observation)
            .map_err(|_| MutationStartError::Storage)?;
        if let Some(authorization) = self.authorization.as_mut() {
            authorization.revoke()?;
        }
        let release = actor_effect(
            registry,
            self.operation,
            Some(DomainInput::Receipt {
                effect: commit,
                receipt: EffectReceipt::MetadataNoChange,
            }),
            EffectSpec::ReleaseResources,
        )?;
        release_mutation_resources(self.operation, self.cleanup.as_ref(), resources, registry)
            .await?;
        actor_effect(
            registry,
            self.operation,
            Some(DomainInput::Receipt {
                effect: release,
                receipt: EffectReceipt::CleanupConfirmed,
            }),
            EffectSpec::RecordTerminal,
        )?;
        carosign_persistence::mutation_store::complete_v4(store, self.mutation, self.operation)
            .map_err(|_| MutationStartError::Storage)?;
        super::actor::publish_mutation_completion(registry, self.operation);
        Ok(MutationReception::RolledBack(reason))
    }

    fn request_rollback(&self) -> Result<[u8; 32], MutationStartError> {
        let command = carosign_linux::ids::KernelIds
            .new_command_id()
            .map_err(|_| MutationStartError::Context)?;
        let timeout = Duration::from_secs(5);
        let deadline = Instant::now() + timeout;
        let epoch = self.epoch.value();
        self.channel
            .request_enrollment_rollback(
                ProviderFrame {
                    major: carosign_protocol::limits::MAJOR,
                    minor: carosign_protocol::limits::MINOR_BASELINE,
                    worker_instance_id: self.worker.as_bytes().to_vec(),
                    frame_seq: 0,
                    command_id: command.as_bytes().to_vec(),
                    operation_id: self.operation.as_bytes().to_vec(),
                    device_epoch: epoch,
                    uid: None,
                    payload: Some(Payload::EnrollmentTransactionRequest(
                        EnrollmentTransactionRequest {
                            mutation_id: self.mutation.as_bytes().to_vec(),
                            enrollment_command_id: self.command.as_bytes().to_vec(),
                            action: EnrollmentTransactionAction::EnrollmentTransactionRollback
                                as i32,
                        },
                    )),
                },
                timeout,
            )
            .map_err(|_| MutationStartError::Provider)?;
        for _ in 0..128 {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(MutationStartError::Provider)?;
            if !self
                .channel
                .has_event(remaining)
                .map_err(|_| MutationStartError::Provider)?
            {
                return Err(MutationStartError::Provider);
            }
            let frame = self
                .channel
                .receive()
                .map_err(|_| MutationStartError::Provider)?;
            if frame.operation_id != self.operation.as_bytes()
                || frame.worker_instance_id != self.worker.as_bytes()
                || frame.device_epoch != epoch
            {
                return Err(MutationStartError::Context);
            }
            match frame.payload.as_ref() {
                Some(Payload::EnrollmentTransactionResult(result))
                    if frame.command_id == command.as_bytes()
                        && result.mutation_id == self.mutation.as_bytes()
                        && result.enrollment_command_id == self.command.as_bytes()
                        && result.state
                            == EnrollmentTransactionState::EnrollmentTransactionRolledBack
                                as i32 =>
                {
                    return Ok(carosign_persistence::mutation_plan::sha256(
                        &frame.encode_to_vec(),
                    ));
                }
                Some(
                    Payload::CommandAck(_)
                    | Payload::ProviderProgress(_)
                    | Payload::MutationOutcome(_),
                ) if frame.command_id == self.command.as_bytes() => {}
                _ => return Err(MutationStartError::Provider),
            }
        }
        Err(MutationStartError::Provider)
    }
}
