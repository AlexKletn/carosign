use carosign_runtime::ports::TimeSource;

use super::LinuxDispatcher;

pub(super) async fn execute_mutation<S, T>(
    dispatcher: &LinuxDispatcher<'_, S, T>,
    connection: &crate::server::Connection,
    handle: crate::operations::ConnectionHandle,
    request_id: &[u8],
    prepared: &crate::mutation_dispatch::PreparedMutation,
) -> Result<(), crate::talk::Ended>
where
    S: carosign_persistence::store::MetadataStore,
    T: TimeSource + 'static,
{
    use prost::Message;
    let version = connection
        .session
        .negotiated()
        .ok_or_else(|| crate::talk::Ended::Refused("version unavailable".into()))?;
    let active = match crate::mutation_execute::start(
        prepared,
        dispatcher.service.store,
        dispatcher.service.authority,
        dispatcher.profile,
        &dispatcher.resources,
        &dispatcher.registry,
        handle,
        connection,
        dispatcher.control.clone(),
        dispatcher.clock.clone(),
    )
    .await
    {
        Ok(active) => active,
        Err(_) => {
            let mut frame = carosign_linux_ipc::codec::error_frame(
                request_id,
                carosign_protocol::carosign::common::v1::ErrorCode::ErrorUnavailable,
                "mutation unavailable",
            );
            frame.major = version.major;
            frame.minor = version.minor;
            return crate::server::write_frame(connection, &frame.encode_to_vec())
                .map_err(crate::talk::Ended::Broken);
        }
    };
    let operation = active.operation;
    let mutation = active.mutation;
    let guard = active
        .control
        .clone()
        .ok_or_else(|| crate::talk::Ended::Refused("mutation control unavailable".into()))?;
    let mut accepted = crate::answer::Reply::Accepted {
        operation,
        boot: prepared.grant.boot(),
        instant_ns: prepared.grant.deadline_ns(),
    }
    .frame(request_id);
    accepted.major = version.major;
    accepted.minor = version.minor;
    if crate::server::write_frame(connection, &accepted.encode_to_vec()).is_err() {
        let _ = guard.reconcile(dispatcher.service.store, mutation);
        return Err(crate::talk::Ended::Refused(
            "mutation client disconnected".into(),
        ));
    }
    let mut next_event_sequence = 1;
    let received = match active
        .receive(
            prepared,
            dispatcher.service.store,
            dispatcher.service.authority,
            &dispatcher.registry,
            &dispatcher.resources,
            |event| {
                use crate::mutation_execute::MutationStartError;
                if guard.interrupted() || event.sequence != next_event_sequence {
                    return Err(MutationStartError::State);
                }
                let mut frame = crate::answer::operation_progress_frame(
                    operation,
                    &event,
                    connection.session.capture_feedback_v1(),
                )
                .ok_or(MutationStartError::State)?;
                frame.major = version.major;
                frame.minor = version.minor;
                crate::server::write_frame(connection, &frame.encode_to_vec())
                    .map_err(|_| MutationStartError::State)?;
                next_event_sequence += 1;
                Ok(())
            },
        )
        .await
    {
        Ok(received) => received,
        Err(error) => {
            let _ = guard.reconcile(dispatcher.service.store, mutation);
            return Err(crate::talk::Ended::Refused(format!(
                "mutation result unavailable: {error:?}"
            )));
        }
    };
    let (rolled_back, rollback_reason) = match &received {
        crate::mutation_execute::MutationReception::RolledBack(reason) => (true, *reason),
        _ => (false, None),
    };
    if let crate::mutation_execute::MutationReception::Observed(observed) = received {
        if guard.interrupted() {
            let _ = guard.reconcile(dispatcher.service.store, mutation);
            return Err(crate::talk::Ended::Refused("mutation interrupted".into()));
        }
        if crate::mutation_execute::finish_guarded(
            observed,
            prepared,
            dispatcher.service.store,
            dispatcher.service.authority,
            &dispatcher.resources,
            &dispatcher.registry,
            &guard,
        )
        .await
        .is_err()
        {
            let _ = guard.reconcile(dispatcher.service.store, mutation);
            return Err(crate::talk::Ended::Refused(
                "mutation completion unavailable".into(),
            ));
        }
        if guard.interrupted() {
            return Err(crate::talk::Ended::Refused(
                "mutation result revoked".into(),
            ));
        }
    }
    let delivery = crate::mutation_delivery::MutationDelivery::new(
        prepared,
        operation,
        mutation,
        next_event_sequence,
    )
    .map_err(|_| crate::talk::Ended::Refused("mutation delivery unavailable".into()))?;
    delivery
        .send(
            dispatcher.service.store,
            dispatcher.service.authority,
            &prepared.caller,
            |mut frame| {
                if !rolled_back && guard.interrupted() {
                    return Err(());
                }
                use carosign_protocol::carosign::client::v1::{
                    MutationResult, client_frame::Payload, terminal_result::Kind,
                };
                if let Some(reason) = rollback_reason
                    && let Some(Payload::OperationEvent(event)) = frame.payload.as_mut()
                    && let Some(Kind::Mutation(result)) = event
                        .terminal
                        .as_mut()
                        .and_then(|terminal| terminal.kind.as_mut())
                    && result.result() == MutationResult::MutationNoChange
                {
                    result.error = Some(carosign_protocol::carosign::common::v1::Error {
                        code: reason as i32,
                        safe_message: String::new(),
                    });
                }
                frame.major = version.major;
                frame.minor = version.minor;
                crate::server::write_frame(connection, &frame.encode_to_vec()).map_err(|_| ())
            },
        )
        .map_err(|_| crate::talk::Ended::Refused("mutation delivery unavailable".into()))
}
