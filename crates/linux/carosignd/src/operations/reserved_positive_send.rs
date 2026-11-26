use super::{Entry, ObservationFailure, OwnerDeliveryFailure, OwnerSendError, OwnerSendProgress};

/// One system call for already authorized delivery, the caller must hold the table of
/// operations and check the recipient and authority before the call.
/// This transition is not in itself a source of trust.
#[allow(dead_code)]
pub(super) fn send_reserved_positive(
    entry: &mut Entry,
    fd: &std::os::fd::OwnedFd,
    bytes: &[u8],
) -> Result<OwnerSendProgress, OwnerSendError> {
    use carosign_runtime::operation::hub::ReservedSendOutcome;
    use std::os::fd::AsRawFd;
    if bytes.is_empty() || bytes.len() > carosign_protocol::limits::FRAME_MAX {
        return Err(OwnerSendError::Socket(libc::EMSGSIZE));
    }
    let reservation = entry
        .positive_reservation
        .as_ref()
        .ok_or(OwnerSendError::Delivery(OwnerDeliveryFailure::Operation(
            ObservationFailure::ContextMismatch,
        )))?;
    let mut hub = entry.events.lock().unwrap_or_else(|e| e.into_inner());
    hub.begin_reserved_send(reservation)
        .map_err(|_| OwnerSendError::Finished)?;
    // SAFETY: The descriptor and buffer live until the end of the non-blocking call.
    let sent = unsafe {
        libc::send(
            fd.as_raw_fd(),
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
    let (outcome, result) = match error {
        Some(libc::EAGAIN) => (
            ReservedSendOutcome::NotSent,
            Ok(OwnerSendProgress::WouldBlock),
        ),
        Some(error) => (
            ReservedSendOutcome::Unknown,
            Err(OwnerSendError::Socket(error)),
        ),
        None if sent as usize == bytes.len() => {
            (ReservedSendOutcome::Sent, Ok(OwnerSendProgress::Sent))
        }
        None => (
            ReservedSendOutcome::Unknown,
            Err(OwnerSendError::Socket(libc::EIO)),
        ),
    };
    hub.finish_reserved_send(reservation, outcome)
        .map_err(|_| OwnerSendError::Finished)?;
    result
}
