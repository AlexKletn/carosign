use super::{
    Asked, ClientFrame, ClientRequest, Connection, ConversationEnd, CountedServeError, Ended,
    IdSource, Message, PolicyAuthority, Service, TimeSource, VerificationDispatcher, answer, codec,
    common, cycle, read_one, send_one, server, wait_for_hello,
};

/// He's got one session till closing.
///
/// ⚠️ The epoch of the device is a property of the apparatus, not of the frame: while it is
/// given by the adapter, and the reassessment of the epoch will come along with the
/// inventory.
pub async fn converse<S, A, I, T>(
    connection: &mut Connection,
    service: &Service<'_, S, A, I, T>,
    asked: Asked,
) -> Ended
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    converse_with_health(
        connection,
        service,
        asked,
        &crate::health::UnavailableHealth,
    )
    .await
}

/// Conducts a session with the source of the current state from the service.
pub async fn converse_with_health<S, A, I, T>(
    connection: &mut Connection,
    service: &Service<'_, S, A, I, T>,
    asked: Asked,
    health: &dyn crate::health::HealthSource,
) -> Ended
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    converse_with_timeout(
        connection,
        service,
        asked,
        health,
        std::time::Duration::from_millis(carosign_linux_ipc::client::session::HELLO_TIMEOUT_MS),
        None,
    )
    .await
}

/// Applies the verified settings to one already accounted connection.
/// Trust in the sources of the service and the publication of the listener does not create
/// this challenge.
#[allow(dead_code)]
pub(crate) async fn converse_configured<S, A, I, T>(
    connection: &mut Connection,
    service: &Service<'_, S, A, I, T>,
    asked: Asked,
    health: &dyn crate::health::HealthSource,
    config: &crate::config::Config,
) -> Ended
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    if config.validate().is_err() {
        return Ended::Refused("invalid configuration".to_string());
    }
    converse_with_timeout(
        connection,
        service,
        asked,
        health,
        std::time::Duration::from_secs(u64::from(config.hello_timeout_s)),
        None,
    )
    .await
}

/// Composition point for one recorded connection with the performer of the check.
/// The publication of the listener and the origin of the dependencies remain with the
/// composition.
#[allow(dead_code)]
pub(crate) async fn converse_counted_with_dispatcher<S, A, I, T>(
    counted: &mut crate::server::CountedConnection<'_>,
    service: &Service<'_, S, A, I, T>,
    asked: Asked,
    health: &dyn crate::health::HealthSource,
    config: &crate::config::Config,
    dispatcher: &dyn VerificationDispatcher,
) -> Ended
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    if config.validate().is_err() {
        return Ended::Refused("invalid configuration".to_string());
    }
    let handle = counted.handle;
    converse_with_timeout(
        &mut counted.connection,
        service,
        asked,
        health,
        std::time::Duration::from_secs(u64::from(config.hello_timeout_s)),
        Some((handle, dispatcher)),
    )
    .await
}

/// Accepts and conducts one recorded session on an already published listener.
///
/// The composition is required to check the recovery, catalogue and policy before
/// publication.
/// The function does not establish readiness or trust in its dependencies.
/// The session does not yet allow operations: the accounting connection with them is not yet
/// connected.
#[allow(dead_code)]
pub(crate) async fn serve_counted_once<S, A, I, T>(
    listener: &std::os::fd::OwnedFd,
    service: &Service<'_, S, A, I, T>,
    asked: Asked,
    health: &dyn crate::health::HealthSource,
    config: &crate::config::Config,
) -> Result<Ended, CountedServeError>
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    config
        .validate()
        .map_err(|_| CountedServeError::InvalidConfig)?;
    let mut counted = server::accept_counted(
        listener,
        service.operations,
        config.ops_per_connection,
        service.ids,
    )
    .map_err(CountedServeError::Accept)?;
    Ok(converse_configured(&mut counted.connection, service, asked, health, config).await)
}

/// Receives and maintains a recorded connection with an expressly transferred performer.
/// The source of the performer and the readiness of the listener confirms the composition of
/// the service.
#[allow(dead_code)]
pub(crate) async fn serve_counted_once_with_dispatcher<S, A, I, T>(
    listener: &std::os::fd::OwnedFd,
    service: &Service<'_, S, A, I, T>,
    registry: std::sync::Arc<crate::operations::OperationRegistry>,
    asked: Asked,
    health: &dyn crate::health::HealthSource,
    config: &crate::config::Config,
    dispatcher: &dyn VerificationDispatcher,
) -> Result<Ended, CountedServeError>
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    if !std::ptr::eq(service.operations, std::sync::Arc::as_ptr(&registry)) {
        return Err(CountedServeError::WatchUnavailable);
    }
    config
        .validate()
        .map_err(|_| CountedServeError::InvalidConfig)?;
    let mut counted = server::accept_counted(
        listener,
        service.operations,
        config.ops_per_connection,
        service.ids,
    )
    .map_err(CountedServeError::Accept)?;
    // A silent client does not hold a single performer indefinitely.
    let timeout = libc::timeval {
        tv_sec: 60,
        tv_usec: 0,
    };
    use std::os::fd::AsRawFd;
    // SAFETY: The socket and timeout structure are valid for the call time.
    if unsafe {
        libc::setsockopt(
            counted.connection.fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            (&timeout as *const libc::timeval).cast(),
            std::mem::size_of_val(&timeout) as libc::socklen_t,
        )
    } != 0
    {
        return Err(CountedServeError::WatchUnavailable);
    }
    let _watch =
        server::OwnerDisconnectWatch::start(&counted.connection.fd, registry, counted.handle)
            .map_err(|_| CountedServeError::WatchUnavailable)?;
    Ok(
        converse_counted_with_dispatcher(&mut counted, service, asked, health, config, dispatcher)
            .await,
    )
}

pub(super) async fn converse_with_timeout<S, A, I, T>(
    connection: &mut Connection,
    service: &Service<'_, S, A, I, T>,
    asked: Asked,
    health: &dyn crate::health::HealthSource,
    hello_timeout: std::time::Duration,
    dispatcher: Option<(
        crate::operations::ConnectionHandle,
        &dyn VerificationDispatcher,
    )>,
) -> Ended
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    // ⚠️ The first message is just a greeting.
    if let Err(error) = wait_for_hello(connection, hello_timeout) {
        return error;
    }
    let first = match read_one(connection) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Ended::Closed,
        Err(error) => return error,
    };
    match server::greet(connection, &first) {
        Ok(answer) => {
            if let Err(error) = send_one(connection, &answer) {
                return error;
            }
        }
        Err(ConversationEnd::Refused(reason)) => {
            // An unsupported greeting can only be answered if the structure and identifier of
            // the request are clearly disassembled.
            let decoded = codec::decode(&first);
            let structurally_safe = matches!(
                decoded,
                Ok(_)
                    | Err(codec::RequestError::Wire(
                        carosign_protocol::validation::WireError::UnsupportedVersion { .. }
                    ))
            );
            if structurally_safe
                && let Ok(raw) = ClientFrame::decode(&first[..])
                && raw.request_id.len() == 16
                && raw.request_id.iter().any(|byte| *byte != 0)
                && let Some(carosign_protocol::carosign::client::v1::client_frame::Payload::Hello(
                    hello,
                )) = raw.payload
                && (raw.major != carosign_protocol::limits::MAJOR
                    || carosign_protocol::version::negotiate(
                        hello.major,
                        hello.min_minor,
                        hello.max_minor,
                    )
                    .is_err())
            {
                let answer = codec::error_frame(
                    &raw.request_id,
                    common::ErrorCode::ErrorUnsupportedVersion,
                    "unsupported version",
                );
                if let Err(broken) = send_one(connection, &answer.encode_to_vec()) {
                    return broken;
                }
            }
            return Ended::Refused(reason);
        }
        Err(ConversationEnd::ClosedEarly) => return Ended::Closed,
    }

    loop {
        let bytes = match read_one(connection) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ended::Closed,
            Err(error) => return error,
        };

        // ⚠️ The session takes the shot itself: one phase and version check, not two.
        let request = match connection.session.accept(&bytes) {
            Ok(request) => request,
            Err(error) => {
                // Unreadable or ambiguous frames don't securely link the response to the
                // request, don't send a frame with an empty ID.
                if matches!(
                    error,
                    carosign_linux_ipc::client::session::SessionError::Request(
                        codec::RequestError::Malformed
                    ) | carosign_linux_ipc::client::session::SessionError::Frame(_)
                ) {
                    return Ended::Refused(error.to_string());
                }
                let raw = ClientFrame::decode(&bytes[..]).ok();
                let Some(frame) = raw.as_ref().filter(|frame| {
                    frame.request_id.len() == 16 && frame.request_id.iter().any(|byte| *byte != 0)
                }) else {
                    return Ended::Refused(error.to_string());
                };
                let wrong_version = connection.session.negotiated().is_none_or(|version| {
                    frame.major != version.major || frame.minor != version.minor
                });
                let recoverable = matches!(
                    error,
                    carosign_linux_ipc::client::session::SessionError::Request(
                        codec::RequestError::Wire(_)
                    ) | carosign_linux_ipc::client::session::SessionError::TooManyOperations
                ) && !wrong_version
                    && !matches!(
                        error,
                        carosign_linux_ipc::client::session::SessionError::Request(
                            codec::RequestError::Wire(
                                carosign_protocol::validation::WireError::UnsupportedVersion { .. }
                            )
                        )
                    );
                let code = match (&error, wrong_version) {
                    (_, true)
                    | (
                        carosign_linux_ipc::client::session::SessionError::Request(
                            codec::RequestError::Wire(
                                carosign_protocol::validation::WireError::UnsupportedVersion {
                                    ..
                                },
                            ),
                        ),
                        _,
                    )
                    | (carosign_linux_ipc::client::session::SessionError::VersionMismatch, _) => {
                        common::ErrorCode::ErrorUnsupportedVersion
                    }
                    (carosign_linux_ipc::client::session::SessionError::TooManyOperations, _) => {
                        common::ErrorCode::ErrorLimitExceeded
                    }
                    _ => common::ErrorCode::ErrorInvalidMessage,
                };
                let answer = codec::error_frame(&frame.request_id, code, "request refused");
                if let Err(broken) = send_one(connection, &answer.encode_to_vec()) {
                    return broken;
                }
                if recoverable {
                    continue;
                }
                return Ended::Refused(error.to_string());
            }
        };

        let request_id = ClientFrame::decode(&bytes[..])
            .map(|frame| frame.request_id)
            .unwrap_or_default();

        if matches!(
            request,
            ClientRequest::BeginVerification { .. }
                | ClientRequest::BeginEnrollment { .. }
                | ClientRequest::BeginRemoval { .. }
        ) && let Some((handle, dispatcher)) = dispatcher
        {
            if let Err(error) = dispatcher
                .dispatch(connection, handle, &request, &request_id)
                .await
            {
                return error;
            }
            continue;
        }

        if let Some((_, dispatcher)) = dispatcher
            && let Some(frame) = super::inventory_response::answer_inventory(
                connection,
                dispatcher,
                &request,
                &request_id,
            )
        {
            let frame = match frame {
                Ok(frame) => frame,
                Err(error) => return error,
            };
            if let Err(error) = send_one(connection, &frame) {
                return error;
            }
            continue;
        }

        if let ClientRequest::ObserveOperation {
            operation,
            after_sequence,
        } = &request
            && let Some((_, dispatcher)) = dispatcher
        {
            let Ok(operation) = carosign_core::ids::OperationId::try_from(operation.as_slice())
            else {
                return Ended::Refused("invalid operation ID".into());
            };
            if let Err(error) =
                dispatcher.observe_operation(connection, operation, *after_sequence, &request_id)
            {
                return error;
            }
            continue;
        }

        if let ClientRequest::GetOperation { operation }
        | ClientRequest::CancelOperation { operation } = &request
            && let Some((_, dispatcher)) = dispatcher
        {
            let frame = match super::operation_status_response::answer(
                connection,
                dispatcher,
                operation,
                matches!(request, ClientRequest::CancelOperation { .. }),
                &request_id,
            ) {
                Ok(frame) => frame,
                Err(error) => return error,
            };
            if let Err(error) = send_one(connection, &frame) {
                return error;
            }
            continue;
        }

        if let ClientRequest::RenameEnrollment(rename) = &request
            && let Some((_, dispatcher)) = dispatcher
        {
            let frame = match dispatcher.rename_enrollment(connection, rename).await {
                Ok(renamed) => {
                    let Some(version) = connection.session.negotiated() else {
                        return Ended::Refused("version unavailable".into());
                    };
                    ClientFrame {
                        major: version.major,
                        minor: version.minor,
                        request_id,
                        operation_id: Vec::new(),
                        event_sequence: 0,
                        payload: Some(
                            carosign_protocol::carosign::client::v1::client_frame::Payload::EnrollmentRenamed(renamed),
                        ),
                    }
                    .encode_to_vec()
                }
                Err(code) => cycle::error_frame(&request_id, code, "rename enrollment refused"),
            };
            if let Err(error) = send_one(connection, &frame) {
                return error;
            }
            continue;
        }

        if matches!(request, ClientRequest::ListEnrollments { .. })
            && let Some((_, dispatcher)) = dispatcher
        {
            let frame = match dispatcher.enrollments(connection, &request).await {
                Ok(entries) => {
                    let Some(version) = connection.session.negotiated() else {
                        return Ended::Refused("version unavailable".into());
                    };
                    ClientFrame {
                        major: version.major,
                        minor: version.minor,
                        request_id,
                        operation_id: Vec::new(),
                        event_sequence: 0,
                        payload: Some(
                            carosign_protocol::carosign::client::v1::client_frame::Payload::Enrollments(
                                carosign_protocol::carosign::client::v1::Enrollments { entries },
                            ),
                        ),
                    }
                    .encode_to_vec()
                }
                Err(()) => cycle::error_frame(
                    &request_id,
                    common::ErrorCode::ErrorUnavailable,
                    "enrollments unavailable",
                ),
            };
            if let Err(error) = send_one(connection, &frame) {
                return error;
            }
            continue;
        }

        let frames = match answer(connection, service, &request, &request_id, asked, health).await {
            Ok(frames) => frames,
            Err(error) => return error,
        };
        for frame in frames {
            if let Err(error) = send_one(connection, &frame) {
                return error;
            }
        }
    }
}
