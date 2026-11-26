use crate::dbus::DeleteError;
use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::{client::v1 as wire, common::v1 as common};
use std::sync::{Arc, Mutex};

use super::{
    SOCKET,
    cancellation::{Cancellation, request_cancel},
    fingerprint_inventory::authenticator_for,
};

pub(super) fn delete_records(
    user: &str,
    device: Option<&[u8]>,
    finger: Option<&str>,
    cancel: &Arc<Mutex<Cancellation>>,
) -> Result<(), DeleteError> {
    use carosign_linux_ipc::client::mutation_events::observe_mutation_with_acceptance;
    let mut failure = DeleteError::PrintsNotDeleted;
    let mut run = || -> Option<()> {
        let selector = || {
            Some(common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::AccountHint(
                    user.to_owned(),
                )),
            })
        };
        let transport = Arc::new(Transport::connect_root(SOCKET).ok()?);
        let authenticator_id = authenticator_for(
            &transport,
            device,
            common::ProviderOperation::ProviderRemove,
        )?;
        let response = transport
            .request(wire::client_frame::Payload::ListEnrollments(
                wire::ListEnrollments {
                    subject: selector(),
                    authenticator_id: authenticator_id.clone(),
                    authority_request_ref: Vec::new(),
                },
            ))
            .ok()?;
        let Some(wire::client_frame::Payload::Enrollments(list)) = response.payload else {
            return None;
        };
        let mut selected = Vec::new();
        for entry in list.entries {
            if finger.is_none_or(|name| name == entry.original_label.as_deref().unwrap_or(&entry.label)) {
                if entry.pending_reconciliation {
                    return None;
                }
                selected.push(entry.enrollment?);
            }
        }
        if selected.is_empty() {
            failure = DeleteError::NoEnrolledPrints;
            return None;
        }
        if finger.is_some() && selected.len() != 1 {
            return None;
        }
        for enrollment in selected {
            if cancel.lock().ok()?.cancelled {
                return None;
            }
            let mut key = [0u8; 16];
            if unsafe { libc::getrandom(key.as_mut_ptr().cast(), key.len(), 0) }
                != key.len() as isize
                || key.iter().all(|byte| *byte == 0)
            {
                return None;
            }
            let operation_transport = transport.clone();
            let worker_cancel = cancel.clone();
            let request = wire::client_frame::Payload::BeginRemoval(wire::BeginRemoval {
                idempotency_key: key.to_vec(),
                subject: selector(),
                enrollment: Some(enrollment.clone()),
                authority_request_ref: Vec::new(),
            });
            let result = observe_mutation_with_acceptance(&transport, request, move |id| {
                let Ok(mut state) = worker_cancel.lock() else {
                    return;
                };
                let operation = (operation_transport, id.to_vec());
                if state.cancelled {
                    drop(state);
                    request_cancel(operation.0, operation.1);
                } else {
                    state.operation = Some(operation);
                }
            })
            .ok()?;
            if result.result != wire::MutationResult::MutationCommitted as i32
                || result.enrollment != Some(enrollment)
            {
                if result.error.is_some_and(|error| {
                    matches!(
                        error.code(),
                        common::ErrorCode::ErrorDeviceRemoved
                            | common::ErrorCode::ErrorResourceUncertain
                    )
                }) {
                    failure = DeleteError::PrintsNotDeletedFromDevice;
                }
                return None;
            }
            cancel.lock().ok()?.operation = None;
        }
        let response = transport
            .request(wire::client_frame::Payload::ListEnrollments(
                wire::ListEnrollments {
                    subject: selector(),
                    authenticator_id,
                    authority_request_ref: Vec::new(),
                },
            ))
            .ok()?;
        let Some(wire::client_frame::Payload::Enrollments(after)) = response.payload else {
            return None;
        };
        if after
            .entries
            .iter()
            .any(|entry| finger.is_none_or(|name| name == entry.original_label.as_deref().unwrap_or(&entry.label)))
        {
            return None;
        }
        Some(())
    };
    run().ok_or(failure)
}
