use crate::client::{Observation, Request, observe_system_verification_with_control};
use crate::result::PamCode;
use carosign_protocol::carosign::common::v1::VerificationPurpose;

pub(super) fn authenticate_user(
    user: &str,
    service: &str,
    localizer: &std::sync::Arc<carosign_l10n::Localizer>,
    mut accepted: impl FnMut(),
) -> PamCode {
    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1));
    loop {
        let (result, retry) =
            authenticate_attempt(user, service, localizer, &attempts, &mut accepted);
        if !retry {
            return result;
        }
    }
}

fn retry_no_sample(service: &str, has_tty: bool, cancelled: bool) -> bool {
    matches!(service, "sudo" | "sudo-i") && has_tty && !cancelled
}

pub(crate) const SUDO_MAX_ATTEMPTS: usize = 3;

fn retry_mismatch(attempts: &std::sync::atomic::AtomicUsize) -> bool {
    attempts
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |current| (current < SUDO_MAX_ATTEMPTS).then_some(current + 1),
        )
        .is_ok()
}

fn authenticate_attempt(
    user: &str,
    service: &str,
    localizer: &std::sync::Arc<carosign_l10n::Localizer>,
    attempts: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
    accepted: &mut impl FnMut(),
) -> (PamCode, bool) {
    let mut key = [0u8; 16];
    // SAFETY: The buffer is valid and is designed for the specified number of bytes.
    let read = unsafe { libc::getrandom(key.as_mut_ptr().cast(), key.len(), 0) };
    if read != key.len() as isize || key.iter().all(|byte| *byte == 0) {
        return (PamCode::AuthInfoUnavail, false);
    }
    let request = Request {
        user: user.into(),
        service: String::new(),
        connection: String::new(),
    };
    let tty = crate::tty_ui::TtyUi::open(service, localizer.clone());
    let retry_empty = retry_no_sample(service, tty.is_some(), false);
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let result = std::thread::scope(|scope| {
        let result = observe_system_verification_with_control(
            "/run/carosignd/control.sock",
            &request,
            key,
            VerificationPurpose::PurposeSessionLogin,
            |transport, operation| {
                if let Some(mut tty) = tty {
                    tty.show(attempts.clone(), operation.deadline.as_ref(), retry_empty);
                    let id = operation.operation_id.clone();
                    let cancelled = &cancelled;
                    scope.spawn(move || {
                    if tty.wait(done_rx) {
                        cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
                        let _ = transport.request(carosign_protocol::carosign::client::v1::client_frame::Payload::CancelOperation(
                            carosign_protocol::carosign::client::v1::CancelOperation { operation_id: id }
                        ));
                    }
                });
                } else {
                    accepted();
                }
            },
            |_| {},
        );
        let _ = done_tx.send(());
        result
    });
    if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
        return (PamCode::AuthInfoUnavail, false);
    }
    match result {
        Ok(Observation::FactorAccepted) => (PamCode::Success, false),
        Ok(Observation::NotMatched) => (PamCode::AuthErr, retry_empty && retry_mismatch(attempts)),
        Ok(Observation::NoSample) => (PamCode::AuthInfoUnavail, retry_empty),
        _ => (PamCode::AuthInfoUnavail, false),
    }
}

#[cfg(test)]
#[path = "authentication.tests.rs"]
mod tests;
