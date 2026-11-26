use super::{OperationRegistry, VerificationDispatchStage};
use carosign_core::ids::OperationId;

/// One-time attempt; loss of holder leaves an uncertain outcome.
#[allow(dead_code)]
pub(crate) struct VerificationSend<'a> {
    pub(super) registry: &'a OperationRegistry,
    pub(super) operation: OperationId,
    pub(super) frame: Option<carosign_protocol::carosign::provider::v1::ProviderFrame>,
    pub(super) permitted: Option<carosign_runtime::PermittedSet>,
    pub(super) finished: bool,
}

#[allow(dead_code)]
impl VerificationSend<'_> {
    #[cfg(test)]
    pub(crate) fn invalidate_frame_for_fixture(&mut self) {
        self.frame.as_mut().expect("одноразовый кадр").payload = None;
    }

    pub(crate) fn send(
        self,
        transport: carosign_linux_ipc::provider::transport::ProviderTransport,
        timeout: std::time::Duration,
    ) -> Result<
        crate::provider_verification::VerificationExchange,
        crate::provider_verification::VerificationError,
    > {
        self.send_channel(
            crate::provider_verification::VerificationTransport::Direct(transport),
            timeout,
        )
    }

    pub(crate) fn send_channel(
        self,
        transport: crate::provider_verification::VerificationTransport,
        timeout: std::time::Duration,
    ) -> Result<
        crate::provider_verification::VerificationExchange,
        crate::provider_verification::VerificationError,
    > {
        self.send_channel_inner(transport, timeout)
    }

    fn send_channel_inner(
        mut self,
        transport: crate::provider_verification::VerificationTransport,
        timeout: std::time::Duration,
    ) -> Result<
        crate::provider_verification::VerificationExchange,
        crate::provider_verification::VerificationError,
    > {
        use crate::provider_verification::{VerificationError, VerificationExchange};
        let revoked = self
            .registry
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&self.operation)
            .is_none_or(|entry| entry.delivery_invalidated || !entry.pending_control.is_empty());
        if revoked {
            self.finish(VerificationDispatchStage::NotSent);
            return Err(VerificationError::Protocol);
        }
        let frame = self.frame.take().expect("одноразовый кадр");
        let permitted = self.permitted.take().expect("одноразовый набор");
        let result = VerificationExchange::start_channel(transport, frame, permitted, timeout);
        let stage = match &result {
            Ok(_) => VerificationDispatchStage::Sent,
            Err(VerificationError::Protocol) => VerificationDispatchStage::NotSent,
            Err(VerificationError::Transport(_) | VerificationError::Finished) => {
                VerificationDispatchStage::OutcomeUnknown
            }
        };
        self.finish(stage);
        result
    }

    fn finish(&mut self, stage: VerificationDispatchStage) {
        let mut entries = self
            .registry
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(entry) = entries.get_mut(&self.operation)
            && entry.verification_dispatch == VerificationDispatchStage::Sending
        {
            entry.verification_dispatch = stage;
        }
        self.finished = true;
    }
}

impl Drop for VerificationSend<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.finish(VerificationDispatchStage::OutcomeUnknown);
        }
    }
}
