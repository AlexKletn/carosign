use super::{
    ObservationFailure, OperationRegistry, OwnedOperation, OwnerDeliveryFailure, OwnerSendError,
    OwnerSendProgress, PreparedOwnerVerification, RetainedOwnerVerification, TerminalRecordStage,
};
use carosign_runtime::ports::CallerContext;

/// Holds the prepared negative outcome until a single entry.
/// Origins `caller`, powers and connections provide the composition of the service.
#[allow(dead_code)]
pub(crate) struct OwnerVerificationDelivery<'a> {
    pub(super) registry: &'a OperationRegistry,
    pub(super) prepared: Option<PreparedOwnerVerification>,
    pub(super) caller: CallerContext,
    pub(super) sending: bool,
    pub(super) cursor: u64,
}

#[allow(dead_code)]
impl<'a> OwnerVerificationDelivery<'a> {
    pub(super) fn new(
        registry: &'a OperationRegistry,
        prepared: PreparedOwnerVerification,
        caller: CallerContext,
    ) -> Self {
        Self {
            registry,
            prepared: Some(prepared),
            caller,
            sending: false,
            cursor: 0,
        }
    }

    /// Every replay before the record asks the policy again. `send` Serialized with a review
    /// in a table; with EAGAIN, the picture remains here, the remaining errors do not allow
    /// automatic resending.
    pub(crate) async fn try_send(
        &mut self,
        registry: &OperationRegistry,
        connection: &crate::server::Connection,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<OwnerSendProgress, OwnerSendError> {
        loop {
            match self
                .try_send_one(registry, connection, authority, clock)
                .await?
            {
                OwnerSendProgress::Advanced => continue,
                other => return Ok(other),
            }
        }
    }

    pub(crate) async fn try_send_one(
        &mut self,
        registry: &OperationRegistry,
        connection: &crate::server::Connection,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<OwnerSendProgress, OwnerSendError> {
        use prost::Message;
        use std::os::fd::AsRawFd;
        if !std::ptr::eq(self.registry, registry) {
            return Err(OwnerSendError::WrongConnection);
        }
        let prepared = self.prepared.as_ref().ok_or(OwnerSendError::Finished)?;
        if connection.session.phase() != carosign_linux_ipc::client::session::SessionPhase::Ready
            || connection.session.connection() != *self.caller.transaction().as_bytes()
            || connection.session.peer().uid() != self.caller.uid()
            || connection.peer.uid != self.caller.uid()
            || connection.session.peer().pid() != connection.peer.pid
            || connection.session.peer().gid() != connection.peer.gid
        {
            return Err(OwnerSendError::WrongConnection);
        }
        let read_grant = |now| {
            let entries = registry.entries.lock().unwrap_or_else(|e| e.into_inner());
            let entry = entries
                .get(&prepared.operation)
                .ok_or(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                    ObservationFailure::UnknownOperation,
                )))?;
            if entry.delivery_invalidated || !entry.pending_control.is_empty() {
                return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Authority(
                    carosign_runtime::ports::AuthorityRefusal::Revoked,
                )));
            }
            let owned = entry.operation.as_ref().ok_or(OwnerSendError::Delivery(
                OwnerDeliveryFailure::Operation(ObservationFailure::Busy),
            ))?;
            let outcome = check_owner_verification(owned, &self.caller, now, true)
                .map_err(OwnerSendError::Delivery)?;
            if outcome != prepared.outcome {
                return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                    ObservationFailure::ContextMismatch,
                )));
            }
            Ok(owned.grant.clone())
        };
        let grant = read_grant(clock.now())?;
        if let Err(error) = authority
            .revalidate_caller(
                &self.caller,
                &grant,
                carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
            )
            .await
        {
            // The inaccessibility can be temporary. The overt renunciation, the recall, and
            // the deadline complete this issuance: the late response of a policy does not
            // animate it.
            if error != carosign_runtime::ports::AuthorityRefusal::Unavailable {
                self.prepared = None;
            }
            return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Authority(
                error,
            )));
        }
        let (bytes, terminal) =
            {
                let entries = registry.entries.lock().unwrap_or_else(|e| e.into_inner());
                let entry = entries
                    .get(&prepared.operation)
                    .ok_or(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                        ObservationFailure::UnknownOperation,
                    )))?;
                let owned = entry.operation.as_ref().ok_or(OwnerSendError::Delivery(
                    OwnerDeliveryFailure::Operation(ObservationFailure::Busy),
                ))?;
                let hub = entry.events.lock().unwrap_or_else(|e| e.into_inner());
                let replay = hub
                    .stream()
                    .replay_from(self.cursor, owned.actor.state())
                    .map_err(|_| {
                        OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                            ObservationFailure::ContextMismatch,
                        ))
                    })?;
                let carosign_runtime::operation::events::Replay::Events(events) = replay else {
                    return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                        ObservationFailure::ContextMismatch,
                    )));
                };
                let event = events.first().ok_or(OwnerSendError::Delivery(
                    OwnerDeliveryFailure::Operation(ObservationFailure::ContextMismatch),
                ))?;
                if self.cursor.checked_add(1) != Some(event.sequence)
                    || event.sequence > prepared.sequence
                {
                    return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                        ObservationFailure::ContextMismatch,
                    )));
                }
                let terminal = event.sequence == prepared.sequence;
                let frame = if terminal
                    && event.kind == carosign_runtime::operation::events::EventKind::Terminal
                {
                    crate::answer::terminal_frame(
                        prepared.operation,
                        prepared.outcome,
                        prepared.sequence,
                    )
                } else if !terminal {
                    crate::answer::operation_progress_frame(
                        prepared.operation,
                        event,
                        connection.session.capture_feedback_v1(),
                    )
                    .ok_or(OwnerSendError::Delivery(
                        OwnerDeliveryFailure::Operation(ObservationFailure::ContextMismatch),
                    ))?
                } else {
                    return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                        ObservationFailure::ContextMismatch,
                    )));
                };
                (frame.encode_to_vec(), terminal)
            };
        let now = clock.now();
        // Do not wait for the table after reading the clock: in employment, the repetition
        // will require a new check of authority and a new point in time.
        let entries = match registry.entries.try_lock() {
            Ok(entries) => entries,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                    ObservationFailure::Busy,
                )));
            }
        };
        let entry = entries
            .get(&prepared.operation)
            .ok_or(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                ObservationFailure::UnknownOperation,
            )))?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Authority(
                carosign_runtime::ports::AuthorityRefusal::Revoked,
            )));
        }
        let owned = entry.operation.as_ref().ok_or(OwnerSendError::Delivery(
            OwnerDeliveryFailure::Operation(ObservationFailure::Busy),
        ))?;
        let outcome = check_owner_verification(owned, &self.caller, now, true)
            .map_err(OwnerSendError::Delivery)?;
        if outcome != prepared.outcome
            || owned.grant.id() != grant.id()
            || owned.grant.issuer() != grant.issuer()
            || owned.grant.scope() != grant.scope()
            || owned.grant.boot() != grant.boot()
            || owned.grant.deadline_ns() != grant.deadline_ns()
        {
            return Err(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
                ObservationFailure::ContextMismatch,
            )));
        }
        // SAFETY: The connection and buffer live until the non-blocking call is completed.
        self.sending = true;
        let sent = unsafe {
            libc::send(
                connection.fd.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        let error = (sent < 0).then(|| {
            std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO)
        });
        drop(entries);
        if error == Some(libc::EAGAIN) {
            self.sending = false;
            return Ok(OwnerSendProgress::WouldBlock);
        }
        self.sending = false;
        match error {
            Some(error) => {
                self.prepared = None;
                Err(OwnerSendError::Socket(error))
            }
            None if sent as usize == bytes.len() && terminal => {
                self.prepared = None;
                Ok(OwnerSendProgress::Sent)
            }
            None if sent as usize == bytes.len() => {
                self.cursor += 1;
                Ok(OwnerSendProgress::Advanced)
            }
            None => {
                self.prepared = None;
                Err(OwnerSendError::Socket(libc::EIO))
            }
        }
    }
}

impl Drop for OwnerVerificationDelivery<'_> {
    fn drop(&mut self) {
        // The recording started may have an unknown outcome and is not replayed.
        if self.sending {
            return;
        }
        let Some(prepared) = self.prepared.take() else {
            return;
        };
        let mut entries = self
            .registry
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = entries.get_mut(&prepared.operation)
            && !entry.delivery_invalidated
            && entry.retained_delivery.is_none()
        {
            entry.retained_delivery = Some(RetainedOwnerVerification {
                prepared,
                caller: self.caller.clone(),
                cursor: self.cursor,
            });
        }
    }
}

pub(super) fn check_owner_verification(
    owned: &OwnedOperation,
    caller: &CallerContext,
    now: carosign_core::operation::context::BootInstant,
    reserved: bool,
) -> Result<carosign_engine::TerminalOutcome, OwnerDeliveryFailure> {
    use carosign_engine::{OperationBody, OperationPhase, TerminalOutcome};
    if &owned.owner != caller {
        return Err(OwnerDeliveryFailure::WrongOwner);
    }
    if owned.delivery_reserved != reserved {
        return Err(OwnerDeliveryFailure::AlreadyReserved);
    }
    let state = owned.actor.state();
    if state.common.context.deadline.is_expired_at(now)
        || now.boot() != owned.grant.boot()
        || now.instant_ns() >= owned.grant.deadline_ns()
    {
        return Err(OwnerDeliveryFailure::Operation(
            ObservationFailure::DeadlineExpired,
        ));
    }
    if !matches!(state.body, OperationBody::Verification(_))
        || state.common.phase != OperationPhase::Terminal
        || owned.actor.has_work()
        || !state.common.pending_effects.is_empty()
        || owned.lease.is_some()
        || state.common.lease.is_some()
        || !matches!(owned.terminal_record_stage, TerminalRecordStage::Recorded { outcome, .. } if Some(outcome) == state.common.terminal)
        || !crate::run::grant_matches_operation(&owned.actor, &owned.grant)
    {
        return Err(OwnerDeliveryFailure::Operation(
            ObservationFailure::ContextMismatch,
        ));
    }
    match state.common.terminal {
        Some(
            outcome @ (TerminalOutcome::NotMatched
            | TerminalOutcome::NoSample
            | TerminalOutcome::Cancelled
            | TerminalOutcome::Unavailable
            | TerminalOutcome::Failed),
        ) => Ok(outcome),
        Some(TerminalOutcome::FactorAccepted) => Err(OwnerDeliveryFailure::EvidenceUnavailable),
        _ => Err(OwnerDeliveryFailure::Operation(
            ObservationFailure::ContextMismatch,
        )),
    }
}
