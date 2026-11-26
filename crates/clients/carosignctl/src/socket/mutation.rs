use super::{
    inventory::{devices, enrollments, subject},
    transport_error::map_error,
};
use carosign_l10n::Localizer;
use carosign_linux_ipc::client::{
    mutation_events::observe_mutation_with_events, transport::Transport,
};
use carosign_protocol::carosign::{
    client::v1::{self as wire, client_frame::Payload},
    common::v1::{EnrollmentRef, ProviderOperation},
};
use carosignctl::client::ClientError;

pub(super) fn enroll(
    path: &str,
    user: &str,
    label: &str,
    device_hint: Option<&str>,
) -> Result<Vec<String>, ClientError> {
    if label.is_empty() || label.len() > carosign_protocol::limits::LABEL_MAX {
        return Err(refused("Invalid enrollment label"));
    }
    if let Some(hint) = device_hint
        && (!matches!(hint.len(), 8 | 32) || !hint.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(refused("Use an 8- or 32-digit hexadecimal device ID"));
    }
    let transport = Transport::connect_root(path).map_err(map_error)?;
    let authenticator_id = only(
        devices(&transport)?.into_iter().filter(|device| {
            device.available
                && device.capabilities.as_ref().is_some_and(|capabilities| {
                    capabilities
                        .operations
                        .contains(&(ProviderOperation::ProviderEnroll as i32))
                })
                && device_hint.is_none_or(|hint| {
                    hex(&device.authenticator_id).starts_with(&hint.to_ascii_lowercase())
                })
        }),
        "No available device supports enrollment",
        "Multiple devices match. Specify a device ID from carosignctl list",
    )?
    .authenticator_id;
    let payload = Payload::BeginEnrollment(wire::BeginEnrollment {
        idempotency_key: request_key()?,
        subject: Some(subject((user != "-").then_some(user))),
        authenticator_id: authenticator_id.clone(),
        label: label.into(),
        replace: None,
        authority_request_ref: Vec::new(),
    });
    run_mutation(&transport, payload, |reference| {
        reference.authenticator_id == authenticator_id
    })
}

pub(super) fn remove(path: &str, id: &str, user: Option<&str>) -> Result<Vec<String>, ClientError> {
    if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(refused(
            "Use the full enrollment ID from carosignctl list enrollments",
        ));
    }
    let transport = Transport::connect_root(path).map_err(map_error)?;
    let mut matching = Vec::new();
    for device in devices(&transport)? {
        let records = enrollments(&transport, user, &device.authenticator_id)?;
        for record in records.entries {
            let reference = record.enrollment.ok_or(ClientError::Unexpected)?;
            if reference.authenticator_id != device.authenticator_id
                || reference.generation == 0
                || reference.enrollment_id.len() != 16
                || reference.enrollment_id.iter().all(|byte| *byte == 0)
            {
                return Err(ClientError::Unexpected);
            }
            if hex(&reference.enrollment_id).eq_ignore_ascii_case(id) {
                matching.push((reference, record.pending_reconciliation));
            }
        }
    }
    let (reference, pending) = only(
        matching.into_iter(),
        "Enrollment not found",
        "Enrollment ID is ambiguous",
    )?;
    if pending {
        return Err(refused("Enrollment requires reconciliation"));
    }
    let payload = Payload::BeginRemoval(wire::BeginRemoval {
        idempotency_key: request_key()?,
        subject: Some(subject(user)),
        enrollment: Some(reference.clone()),
        authority_request_ref: Vec::new(),
    });
    run_mutation(&transport, payload, |removed| removed == &reference)
}

fn run_mutation(
    transport: &Transport,
    payload: Payload,
    matches_reference: impl FnOnce(&EnrollmentRef) -> bool,
) -> Result<Vec<String>, ClientError> {
    let localizer = Localizer::from_environment();
    let result = observe_mutation_with_events(
        transport,
        payload,
        |id| eprintln!("operation_id={}", hex(id)),
        |event| {
            let key = match event.kind() {
                wire::EventKind::EventPlaceFinger => "place-finger",
                wire::EventKind::EventMoveFinger => "move-finger",
                wire::EventKind::EventRemoveFinger => "remove-finger",
                wire::EventKind::EventRetry => "retry-capture",
                wire::EventKind::EventProgress => {
                    eprintln!("{}%", event.percent);
                    return;
                }
                _ => return,
            };
            eprintln!("{}", localizer.text(key));
        },
    )
    .map_err(map_error)?;
    match result.result() {
        wire::MutationResult::MutationCommitted => {
            let reference = result.enrollment.ok_or(ClientError::Unexpected)?;
            if !matches_reference(&reference) {
                return Err(ClientError::Unexpected);
            }
            Ok(vec![hex(&reference.enrollment_id)])
        }
        wire::MutationResult::MutationNeedsReconciliation => Err(refused(&format!(
            "Operation requires reconciliation (mutation_id={})",
            hex(&result.mutation_id)
        ))),
        wire::MutationResult::MutationNoChange => {
            Err(refused("Operation finished without changes"))
        }
        wire::MutationResult::MutationFailed => Err(refused("Operation failed")),
        _ => Err(ClientError::Unexpected),
    }
}

fn request_key() -> Result<Vec<u8>, ClientError> {
    let key = carosign_linux::sys::random_16().map_err(|_| ClientError::Unavailable)?;
    if key == [0; 16] {
        return Err(ClientError::Unavailable);
    }
    Ok(key.to_vec())
}

fn only<T>(
    mut values: impl Iterator<Item = T>,
    missing: &str,
    ambiguous: &str,
) -> Result<T, ClientError> {
    let first = values.next().ok_or_else(|| refused(missing))?;
    if values.next().is_some() {
        return Err(refused(ambiguous));
    }
    Ok(first)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn refused(message: &str) -> ClientError {
    ClientError::Refused {
        message: message.into(),
    }
}
