use super::*;

/// Takes the picture apart and tells you what to do.
///
/// ⚠️ `CallerContext` It is collected ** from the session **, not from the frame: the login
/// system request number and uid come from the connection.
pub fn handle(
    session: &ClientSession,
    frame: &[u8],
) -> Result<(ClientRequest, CallerContext), CycleError> {
    let (_, request) = codec::decode(frame).map_err(|_| CycleError::Malformed)?;
    let caller = caller_from_session(session).ok_or(CycleError::Malformed)?;
    Ok((request, caller))
}

/// Answers a request for verification.
///
/// ⚠️ The era of the device is taken ** from the session-communication **, not from the
/// frame: the frame does not know what device it is running on.
pub async fn verification<S, A, I, T>(
    service: &Service<'_, S, A, I, T>,
    session: &ClientSession,
    request_id: &[u8],
    subject: SubjectRef,
    authenticator: AuthenticatorId,
    epoch: DeviceEpoch,
) -> Handled
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    let Some(caller) = caller_from_session(session) else {
        return Handled::Refused(error_frame(
            request_id,
            common::ErrorCode::ErrorUnavailable,
            "connection unavailable",
        ));
    };
    match service.verify(&caller, subject, authenticator, epoch).await {
        Served::Refused(reply) => Handled::Refused(encode(&reply, request_id)),
        Served::Ran {
            operation,
            deadline,
            outcome,
        } => {
            // ⚠️ Time is the one that is written in the operation, not “now plus something”:
            // the client should see exactly what the engine sees.
            let accepted = Reply::Accepted {
                operation,
                boot: deadline.boot(),
                instant_ns: deadline.instant_ns(),
            };
            let terminal = if matches!(outcome, crate::run::RunOutcome::Terminal(_)) {
                match service
                    .prepare_completed_verification(&caller, operation)
                    .await
                {
                    Ok(prepared) => {
                        let (confirmed_operation, confirmed_outcome, sequence) =
                            prepared.into_parts();
                        terminal_if_complete(
                            confirmed_operation,
                            &crate::run::RunOutcome::Terminal(confirmed_outcome),
                            sequence,
                        )
                    }
                    Err(_) => None,
                }
            } else {
                None
            };
            Handled::Started {
                accepted: encode(&accepted, request_id),
                terminal,
            }
        }
    }
}

pub(super) fn terminal_if_complete(
    operation: carosign_core::ids::OperationId,
    outcome: &crate::run::RunOutcome,
    sequence: u64,
) -> Option<Vec<u8>> {
    match outcome {
        crate::run::RunOutcome::Terminal(result) if sequence != 0 => {
            Some(terminal_frame(operation, *result, sequence).encode_to_vec())
        }
        crate::run::RunOutcome::Waiting { .. }
        | crate::run::RunOutcome::StepLimit { .. }
        | crate::run::RunOutcome::Refused { .. }
        | crate::run::RunOutcome::Terminal(_) => None,
    }
}
