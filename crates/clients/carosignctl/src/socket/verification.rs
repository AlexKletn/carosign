use super::{
    inventory::{devices, enrollments, subject},
    transport_error::map_error,
};
use carosign_l10n::Localizer;
use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::client::v1 as wire;
use carosign_protocol::carosign::common::v1 as common;
use carosignctl::client::ClientError;

pub(super) fn verify(path: &str, user: &str) -> Result<Vec<String>, ClientError> {
    if user.is_empty() {
        return Err(ClientError::Refused {
            message: Localizer::from_environment().text("verify-user-required"),
        });
    }
    let transport = Transport::connect_root(path).map_err(map_error)?;
    verify_connected(&transport, user)
}

pub(super) fn verify_connected(
    transport: &Transport,
    user: &str,
) -> Result<Vec<String>, ClientError> {
    use carosign_linux_ipc::client::verification_events::{
        Observation, observe_verification_with_progress,
    };
    let localizer = Localizer::from_environment();
    let candidates: Vec<_> = devices(transport)?
        .into_iter()
        .filter(|device| {
            device.available
                && device.capabilities.as_ref().is_some_and(|c| {
                    c.operations
                        .contains(&(common::ProviderOperation::ProviderVerify as i32))
                })
        })
        .collect();
    let mut selected = None;
    for device in candidates {
        let has_enrollment = enrollments(transport, Some(user), &device.authenticator_id)?
            .entries
            .iter()
            .any(|e| {
                !e.pending_reconciliation
                    && e.enrollment.as_ref().is_some_and(|enrollment| {
                        enrollment.authenticator_id == device.authenticator_id
                            && enrollment.enrollment_id.len() == 16
                            && enrollment.enrollment_id.iter().any(|v| *v != 0)
                            && enrollment.generation != 0
                    })
            });
        if has_enrollment {
            if selected.is_some() {
                return Err(ClientError::Refused {
                    message: localizer.text("verify-multiple-devices"),
                });
            }
            selected = Some(device.authenticator_id);
        }
    }
    let authenticator_id = selected.ok_or(ClientError::Refused {
        message: localizer.text("verify-no-device"),
    })?;
    let mut key = [0u8; 16];
    let count = unsafe { libc::getrandom(key.as_mut_ptr().cast(), key.len(), 0) };
    if count != key.len() as isize || key.iter().all(|v| *v == 0) {
        return Err(ClientError::Unavailable);
    }
    let result = observe_verification_with_progress(
        transport,
        wire::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: key.to_vec(),
            subject: Some(subject(Some(user))),
            authenticator_id,
            purpose_hint: common::VerificationPurpose::PurposeSelfCheck as i32,
            authority_request_ref: Vec::new(),
        },
        |_| {},
        |event| {
            let key = match event {
                wire::EventKind::EventPlaceFinger => "place-finger",
                wire::EventKind::EventRetry => "retry-capture",
                _ => return,
            };
            eprintln!("{}", localizer.text(key));
        },
    )
    .map_err(map_error)?;
    match result {
        Observation::FactorAccepted => Ok(vec![localizer.text("verify-accepted")]),
        Observation::NotMatched => Err(ClientError::Refused {
            message: localizer.text("verify-not-matched"),
        }),
        Observation::NoSample => Err(ClientError::Refused {
            message: localizer.text("verify-no-sample"),
        }),
        Observation::Cancelled => Err(ClientError::Refused {
            message: localizer.text("verify-cancelled"),
        }),
        Observation::Unavailable => Err(ClientError::Unavailable),
        Observation::Failed => Err(ClientError::Refused {
            message: localizer.text("verify-failed"),
        }),
    }
}
