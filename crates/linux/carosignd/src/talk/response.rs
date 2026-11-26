use super::{
    Asked, ClientFrame, ClientRequest, Connection, Ended, Handled, IdSource, Message,
    PolicyAuthority, Service, TimeSource, common, cycle,
};

/// What is wrong with a request that does not lead this way?
///
/// ⚠️ The word for the magazine is English: the magazine screens everything outside of ASCII,
/// and the Russian line comes out in an unreadable stream of bytes.
pub(super) fn unsupported(request: &ClientRequest) -> &'static str {
    match request {
        ClientRequest::Hello { .. } => "second hello",
        ClientRequest::BeginVerification { .. } => "begin verification",
        ClientRequest::ListAuthenticators => "list authenticators",
        ClientRequest::GetAuthenticator { .. } => "get authenticator",
        ClientRequest::RenameEnrollment(_) => "rename enrollment",
        ClientRequest::ListEnrollments { .. } => "list enrollments",
        ClientRequest::BeginEnrollment { .. } => "begin enrollment",
        ClientRequest::BeginRemoval { .. } => "begin removal",
        ClientRequest::ObserveOperation { .. } => "observe operation",
        ClientRequest::GetOperation { .. } => "get operation",
        ClientRequest::CancelOperation { .. } => "cancel operation",
        ClientRequest::GetHealth => "get health",
    }
}

/// What to answer the request: zero, one or two frames.
pub(super) async fn answer<S, A, I, T>(
    _connection: &Connection,
    _service: &Service<'_, S, A, I, T>,
    request: &ClientRequest,
    request_id: &[u8],
    _asked: Asked,
    health: &dyn crate::health::HealthSource,
) -> Result<Vec<Vec<u8>>, Ended>
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    let handled = match request {
        ClientRequest::BeginVerification { .. } => {
            // Trust sources do not yet take the entire context of the request.
            // You can’t set up a given area and goal.
            Handled::Refused(cycle::error_frame(
                request_id,
                common::ErrorCode::ErrorUnavailable,
                "verification context unavailable",
            ))
        }
        ClientRequest::GetHealth => match health.snapshot() {
            Ok(snapshot) => {
                let negotiated = _connection
                    .session
                    .negotiated()
                    .ok_or_else(|| Ended::Refused("negotiated version unavailable".to_string()))?;
                let frame = ClientFrame {
                    major: negotiated.major,
                    minor: negotiated.minor,
                    request_id: request_id.to_vec(),
                    operation_id: Vec::new(),
                    event_sequence: 0,
                    payload: Some(
                        carosign_protocol::carosign::client::v1::client_frame::Payload::Health(
                            snapshot.into_wire(),
                        ),
                    ),
                };
                Handled::Answer(frame.encode_to_vec())
            }
            Err(_) => Handled::Refused(cycle::error_frame(
                request_id,
                common::ErrorCode::ErrorUnavailable,
                "health unavailable",
            )),
        },
        ClientRequest::ListAuthenticators => {
            // The sources of wealth and policy are not yet connected. You can't give out zero
            // inventory or unconfirmed readiness.
            Handled::Refused(cycle::error_frame(
                request_id,
                common::ErrorCode::ErrorUnavailable,
                "diagnostics unavailable",
            ))
        }
        other => Handled::Answer(cycle::error_frame(
            request_id,
            common::ErrorCode::ErrorUnsupported,
            unsupported(other),
        )),
    };

    Ok(match handled {
        Handled::Silent => Vec::new(),
        Handled::Answer(frame) | Handled::Refused(frame) => vec![frame],
        // ⚠️ Acceptance - **early ** of the result: the client must know the number before
        // the number is signed.
        Handled::Started { accepted, terminal } => {
            let mut frames = vec![accepted];
            frames.extend(terminal);
            frames
        }
    })
}
