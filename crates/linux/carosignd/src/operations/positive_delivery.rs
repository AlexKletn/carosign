use super::{
    CleanupStage, Entry, ObservationFailure, OperationRegistry, OwnerDeliveryFailure,
    OwnerSendError, OwnerSendProgress, send_reserved_positive,
};
use carosign_core::ids::OperationId;
use carosign_runtime::ports::CallerContext;

/// The reserve and the cursor belong to the same holder: the lost shipment started will not
/// be repeated.
/// The origin of the connection and the source of the policy ensures the composition of the
/// service.
#[allow(dead_code)]
pub(crate) struct PositiveDeliveryOwner<'a> {
    registry: &'a OperationRegistry,
    operation: OperationId,
    caller: CallerContext,
    effect: carosign_engine::PlannedEffect,
    sequence: u64,
    cursor: u64,
    finished: bool,
    pub(crate) report_matched_enrollment: bool,
    pub(crate) scoped_match: Option<carosign_core::enrollment::EnrollmentRef>,
}

#[allow(dead_code)]
impl<'a> PositiveDeliveryOwner<'a> {
    pub(crate) fn reserve(
        registry: &'a OperationRegistry,
        operation: OperationId,
        caller: CallerContext,
        effect: carosign_engine::PlannedEffect,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<Self, OwnerDeliveryFailure> {
        let sequence = registry.reserve_positive_delivery(operation, &caller, effect, now)?;
        Ok(Self {
            registry,
            operation,
            caller,
            effect,
            sequence,
            cursor: 0,
            finished: false,
            report_matched_enrollment: false,
            scoped_match: None,
        })
    }

    /// Sends no more than one packet; repeat after EAGAIN checks the authority again. Unknown
    /// outcome blocks the repeat at the reserve itself.
    pub(crate) async fn try_send_one(
        &mut self,
        connection: &crate::server::Connection,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<OwnerSendProgress, OwnerSendError> {
        use carosign_engine::{EffectSpec, OperationBody, OperationPhase, TerminalOutcome};
        use prost::Message;
        use std::os::fd::AsRawFd;
        if self.finished {
            return Err(OwnerSendError::Finished);
        }
        if connection.session.phase() != carosign_linux_ipc::client::session::SessionPhase::Ready
            || connection.session.connection() != *self.caller.transaction().as_bytes()
            || connection.session.peer().uid() != self.caller.uid()
            || connection.peer.uid != self.caller.uid()
            || connection.session.peer().pid() != connection.peer.pid
            || connection.session.peer().gid() != connection.peer.gid
        {
            return Err(OwnerSendError::WrongConnection);
        }
        let check = |entry: &Entry, now: carosign_core::operation::context::BootInstant| {
            if entry.delivery_invalidated || !entry.pending_control.is_empty() {
                return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Authority(
                    carosign_runtime::ports::AuthorityRefusal::Revoked,
                )));
            }
            let owned = entry.operation.as_ref().ok_or(OwnerSendError::Delivery(
                OwnerDeliveryFailure::Operation(ObservationFailure::Busy),
            ))?;
            let state = owned.actor.state();
            if !owned.belongs_to(&self.caller)
                || state.common.phase != OperationPhase::Finalizing
                || state.common.terminal != Some(TerminalOutcome::FactorAccepted)
                || !matches!(state.body, OperationBody::Verification(_))
                || state.common.cancel_requested
                || owned.lease.is_some()
                || state.common.lease.is_some()
                || owned.cleanup_stage != CleanupStage::Released
                || owned.actor.has_work()
                || owned.actor.has_control_work()
                || self.effect.spec != EffectSpec::DeliverOutcome
                || !owned.actor.pending().contains(&self.effect)
                || !crate::run::grant_matches_operation(&owned.actor, &owned.grant)
                || entry.positive_reservation.as_ref().map(|r| r.sequence()) != Some(self.sequence)
            {
                return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                    ObservationFailure::ContextMismatch,
                )));
            }
            if state.common.context.deadline.is_expired_at(now)
                || owned.grant.boot() != now.boot()
                || owned.grant.deadline_ns() <= now.instant_ns()
            {
                return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                    ObservationFailure::DeadlineExpired,
                )));
            }
            Ok(owned.grant.clone())
        };
        let grant = {
            let entries = self
                .registry
                .entries
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let entry = entries
                .get(&self.operation)
                .ok_or(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                    ObservationFailure::UnknownOperation,
                )))?;
            check(entry, clock.now())?
        };
        authority
            .revalidate_caller(
                &self.caller,
                &grant,
                carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
            )
            .await
            .map_err(|error| OwnerSendError::Delivery(OwnerDeliveryFailure::Authority(error)))?;
        let now = clock.now();
        let mut entries = self.registry.entries.try_lock().map_err(|_| {
            OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(ObservationFailure::Busy))
        })?;
        let entry = entries
            .get_mut(&self.operation)
            .ok_or(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                ObservationFailure::UnknownOperation,
            )))?;
        let current = check(entry, now)?;
        if current.id() != grant.id()
            || current.issuer() != grant.issuer()
            || current.scope() != grant.scope()
            || current.boot() != grant.boot()
            || current.deadline_ns() != grant.deadline_ns()
        {
            return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                ObservationFailure::ContextMismatch,
            )));
        }
        if self.cursor.checked_add(1) == Some(self.sequence) {
            use carosign_protocol::carosign::client::v1::{
                client_frame::Payload, terminal_result::Kind,
            };
            let mut frame = crate::answer::terminal_frame(
                self.operation,
                TerminalOutcome::FactorAccepted,
                self.sequence,
            );
            if self.report_matched_enrollment
                && matches!(
                    current.scope(),
                    carosign_core::authorization::GrantScope::Verify {
                        purpose:
                            carosign_core::operation::context::VerificationPurpose::SessionLogin,
                        ..
                    }
                )
                && let Some(reference) = self.scoped_match.or(entry.matched_enrollment)
                && let Some(Payload::OperationEvent(event)) = frame.payload.as_mut()
                && let Some(Kind::Verification(result)) = event
                    .terminal
                    .as_mut()
                    .and_then(|terminal| terminal.kind.as_mut())
            {
                result.matched_enrollment =
                    Some(carosign_protocol::carosign::common::v1::EnrollmentRef {
                        enrollment_id: reference.id.as_bytes().to_vec(),
                        authenticator_id: reference.authenticator.as_bytes().to_vec(),
                        generation: reference.generation.value(),
                    });
            }
            let bytes = frame.encode_to_vec();
            let result = send_reserved_positive(entry, &connection.fd, &bytes);
            if !matches!(result, Ok(OwnerSendProgress::WouldBlock)) {
                self.finished = true;
            }
            return result;
        }
        let bytes = {
            let owned = entry.operation.as_ref().ok_or(OwnerSendError::Finished)?;
            let hub = entry.events.lock().unwrap_or_else(|e| e.into_inner());
            let carosign_runtime::operation::events::Replay::Events(events) = hub
                .stream()
                .replay_from(self.cursor, owned.actor.state())
                .map_err(|_| OwnerSendError::Finished)?
            else {
                return Err(OwnerSendError::Finished);
            };
            let event = events.first().ok_or(OwnerSendError::Finished)?;
            if self.cursor.checked_add(1) != Some(event.sequence) || event.sequence >= self.sequence
            {
                return Err(OwnerSendError::Finished);
            }
            crate::answer::operation_progress_frame(
                self.operation,
                event,
                connection.session.capture_feedback_v1(),
            )
            .ok_or(OwnerSendError::Finished)?
            .encode_to_vec()
        };
        // SAFETY: The descriptor and buffer remain valid for the nonblocking write.
        let sent = unsafe {
            libc::send(
                connection.fd.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        if sent < 0 {
            let error = std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO);
            if error == libc::EAGAIN {
                return Ok(OwnerSendProgress::WouldBlock);
            }
            self.finished = true;
            return Err(OwnerSendError::Socket(error));
        }
        if sent as usize != bytes.len() {
            self.finished = true;
            return Err(OwnerSendError::Socket(libc::EIO));
        }
        self.cursor += 1;
        Ok(OwnerSendProgress::Advanced)
    }
}
