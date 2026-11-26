use std::{sync::mpsc, time::Duration};

use super::{SignalError, revocation_signal::RevocationSignal};

/// The repeat of the already waiting review is combined with it; the absence of an observer
/// is reported to the caller.
#[derive(Clone)]
pub(crate) struct RevocationSender(pub(super) mpsc::SyncSender<RevocationSignal>);

impl RevocationSender {
    pub fn revoke(&self) -> Result<(), SignalError> {
        match self.0.try_send(RevocationSignal::Revoked) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(SignalError::Closed),
        }
    }

    /// Returns the confirmation only after the revocation of the result and the control step
    /// are saved by the surveillance owner.
    pub fn cancel_and_wait(&self, timeout: Duration) -> Result<(), SignalError> {
        let (ack, response) = mpsc::sync_channel(1);
        self.0
            .try_send(RevocationSignal::ClientCancel(ack))
            .map_err(|_| SignalError::Closed)?;
        match response.recv_timeout(timeout) {
            Ok(true) => Ok(()),
            _ => Err(SignalError::Closed),
        }
    }
}
