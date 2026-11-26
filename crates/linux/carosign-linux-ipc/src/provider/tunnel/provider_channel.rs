use std::{sync::Mutex, time::Duration};

use carosign_protocol::carosign::{broker_relay::v1::Scope, provider::v1::ProviderFrame};

use super::{TunnelClient, TunnelError};

/// The runtime channel is on top of the already issued tunnel. The channel itself does not
/// establish the origin of the broker or issue authority.
///
/// Cancellation remains closed until scoped control is connected and the individual
/// cancellation owner is connected; no receipt is considered a successful cancellation.
pub struct TunnelProviderChannel {
    client: Mutex<TunnelClient>,
    timeout: Duration,
}

impl TunnelProviderChannel {
    /// Transmits a ready-made tunnel and a limited period of one exchange.
    pub fn new(client: TunnelClient, timeout: Duration) -> Self {
        Self {
            client: Mutex::new(client),
            timeout,
        }
    }

    /// Receives the following verified worker event.
    pub fn receive(&self) -> Result<ProviderFrame, TunnelError> {
        self.client
            .lock()
            .map_err(|_| TunnelError::Closed)?
            .receive_event(self.timeout)
    }

    pub fn has_event(&self, timeout: Duration) -> Result<bool, TunnelError> {
        let client = self.client.lock().map_err(|_| TunnelError::Closed)?;
        client.has_event(timeout)
    }

    pub fn request_enrollment_rollback(
        &self,
        frame: ProviderFrame,
        timeout: Duration,
    ) -> Result<(), TunnelError> {
        self.client
            .lock()
            .map_err(|_| TunnelError::Closed)?
            .command(frame, timeout)
    }

    /// Transfers the area verified by the owner to a separate call manager.
    /// The receipt confirms the dispatch, but not the stop of the provider.
    pub fn cancel_scoped(&self, scope: Scope, cancel_id: [u8; 16]) -> Result<(), TunnelError> {
        self.client
            .lock()
            .map_err(|_| TunnelError::Closed)?
            .cancel_scoped(scope, cancel_id, self.timeout)
    }
}

impl carosign_adapters::provider::ProviderChannel for TunnelProviderChannel {
    fn send(
        &self,
        frame: ProviderFrame,
    ) -> Result<(), carosign_adapters::provider::ProviderFailure> {
        use carosign_adapters::provider::ProviderFailure;
        let mut client = self.client.lock().map_err(|_| ProviderFailure::Closed)?;
        client
            .command(frame, self.timeout)
            .map_err(|error| match error {
                TunnelError::Closed => ProviderFailure::Closed,
                // An incorrect answer may have come after the command has been sent; it
                // cannot be passed off as a refusal before it has been sent.
                TunnelError::Invalid | TunnelError::Io(_) | TunnelError::Timeout => {
                    ProviderFailure::SendFailed
                }
            })
    }

    fn cancel(&self, _operation: carosign_core::ids::OperationId) -> bool {
        false
    }

    fn send_mutation(
        &self,
        frame: ProviderFrame,
        ticket: carosign_persistence::journal::ArmedMutationTicket,
        context: carosign_runtime::ports::MutationArmRequest,
    ) -> carosign_runtime::ports::DispatchOutcome {
        use carosign_runtime::ports::DispatchOutcome;
        if !context.matches_ticket(&ticket) {
            return DispatchOutcome::NotSent;
        }
        let Ok(mut client) = self.client.lock() else {
            return DispatchOutcome::Unknown;
        };
        match client.mutation(frame, ticket, context, self.timeout) {
            Ok(()) => DispatchOutcome::Accepted,
            Err(_) => DispatchOutcome::Unknown,
        }
    }

    fn send_continuation(
        &self,
        frame: ProviderFrame,
        ticket: carosign_persistence::journal::ArmedMutationTicket,
        context: carosign_runtime::ports::MutationArmRequest,
        boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> carosign_runtime::ports::DispatchOutcome {
        use carosign_runtime::ports::DispatchOutcome;
        if !context.matches_ticket(&ticket) {
            return DispatchOutcome::NotSent;
        }
        let Ok(mut client) = self.client.lock() else {
            return DispatchOutcome::Unknown;
        };
        match client.mutation_with_boundary(frame, ticket, context, self.timeout, boundary) {
            Ok(()) => DispatchOutcome::Accepted,
            Err(_) => DispatchOutcome::Unknown,
        }
    }

    fn cancel_scoped(
        &self,
        lease: &carosign_runtime::ports::ResourceLease,
        command: carosign_core::ids::CommandId,
    ) -> carosign_runtime::ports::DispatchOutcome {
        use carosign_core::resource::AccessMode;
        use carosign_protocol::carosign::broker_relay::v1::{
            AccessMode as WireAccessMode, Claim, Owner,
        };

        let owner = lease.owner();
        let scope = Scope {
            lease_id: lease.lease_id().as_bytes().to_vec(),
            owner: Some(Owner {
                operation_id: owner.operation.as_bytes().to_vec(),
                worker_instance: owner.worker.as_bytes().to_vec(),
                authenticator_id: owner.authenticator.id.as_bytes().to_vec(),
                device_epoch: owner.authenticator.device_epoch.value(),
            }),
            claims: lease
                .resource_set()
                .claims()
                .iter()
                .map(|claim| Claim {
                    resource_id: claim.id.as_bytes().to_vec(),
                    mode: match claim.mode {
                        AccessMode::Exclusive => WireAccessMode::Exclusive,
                        AccessMode::SharedRead => WireAccessMode::SharedRead,
                        AccessMode::Mediated => WireAccessMode::Mediated,
                    } as i32,
                })
                .collect(),
        };
        match TunnelProviderChannel::cancel_scoped(self, scope, *command.as_bytes()) {
            Ok(()) => carosign_runtime::ports::DispatchOutcome::Accepted,
            // The error may have occurred after the transmission; repetition is not
            // permitted.
            Err(_) => carosign_runtime::ports::DispatchOutcome::Unknown,
        }
    }
}
