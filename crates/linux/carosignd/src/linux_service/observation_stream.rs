use std::{
    os::fd::AsRawFd,
    time::{Duration, Instant},
};

use carosign_core::{identity::SubjectRef, ids::OperationId};
use carosign_linux_ipc::codec::error_frame;
use carosign_protocol::carosign::{
    client::v1::{self as wire, ClientFrame, OperationSnapshot, client_frame::Payload},
    common::v1::ErrorCode,
};
use carosign_runtime::{
    operation::{
        events::{Event, EventKind},
        hub::AttachError,
    },
    ports::AuthorityRefusal,
};
use prost::Message;

use super::{operation_status::OperationStatus, operation_subscription::OperationSubscription};
use crate::{
    server::{Connection, write_frame},
    talk::Ended,
};

pub(super) fn observe(
    status: &OperationStatus<'_>,
    connection: &Connection,
    operation: OperationId,
    after: u64,
    request_id: &[u8],
) -> Result<(), Ended> {
    let caller = crate::cycle::caller_from_session(&connection.session)
        .ok_or_else(|| Ended::Refused("invalid observer".into()))?;
    let uid = caller.uid();
    let mut snapshot = match status.read(uid, operation) {
        Ok(snapshot) => snapshot,
        Err(error) => return refuse(connection, request_id, authority_error(error)),
    };
    let Some(registry) = status.directory.registry(operation) else {
        if snapshot.last_event_sequence != 0 && after > snapshot.last_event_sequence {
            return refuse(connection, request_id, ErrorCode::ErrorInvalidArgument);
        }
        send_snapshot(connection, request_id, snapshot.clone())?;
        if after < snapshot.last_event_sequence {
            send(connection, gap_frame(operation, after, snapshot))?;
        }
        return Ok(());
    };
    let Some(live) = registry.live_status(operation) else {
        return refuse(connection, request_id, ErrorCode::ErrorUnavailable);
    };
    if let Err(error) = status.policy.authorize_observation(uid, Some(live.subject)) {
        return refuse(connection, request_id, authority_error(error));
    }
    let hub = registry
        .event_hub(operation)
        .map_err(|_| Ended::Refused("operation unavailable".into()))?;
    let mut subscription = None;
    for _ in 0..3 {
        if after > snapshot.last_event_sequence {
            return refuse(connection, request_id, ErrorCode::ErrorInvalidArgument);
        }
        match OperationSubscription::attach(
            hub.clone(),
            snapshot.last_event_sequence,
            after,
            status.observer_limit,
        ) {
            Ok(value) => {
                subscription = Some(value);
                break;
            }
            Err(AttachError::CatchupRequired) => {
                snapshot = match status.read(uid, operation) {
                    Ok(snapshot) => snapshot,
                    Err(error) => return refuse(connection, request_id, authority_error(error)),
                };
            }
            Err(AttachError::Limit) => {
                return refuse(connection, request_id, ErrorCode::ErrorLimitExceeded);
            }
            Err(_) => return refuse(connection, request_id, ErrorCode::ErrorUnavailable),
        }
    }
    let Some((subscription, replay)) = subscription else {
        return refuse(connection, request_id, ErrorCode::ErrorBusy);
    };
    let owner = std::sync::Arc::downgrade(&registry);
    drop(registry);
    serve_subscription(
        status,
        connection,
        uid,
        live.subject,
        operation,
        request_id,
        after,
        snapshot,
        subscription,
        replay,
        owner,
    )
}

#[allow(clippy::too_many_arguments)]
fn serve_subscription(
    status: &OperationStatus<'_>,
    connection: &Connection,
    uid: u32,
    subject: SubjectRef,
    operation: OperationId,
    request_id: &[u8],
    after: u64,
    snapshot: OperationSnapshot,
    subscription: OperationSubscription,
    replay: Option<Vec<Event>>,
    registry: std::sync::Weak<crate::operations::OperationRegistry>,
) -> Result<(), Ended> {
    let authorize = || {
        status
            .policy
            .authorize_observation(uid, Some(subject))
            .map_err(|_| Ended::Closed)
    };
    authorize()?;
    send_snapshot(connection, request_id, snapshot.clone())?;
    if let Some(events) = replay {
        for event in events {
            authorize()?;
            send_event(connection, operation, event, &snapshot)?;
        }
    } else {
        authorize()?;
        send(connection, gap_frame(operation, after, snapshot.clone()))?;
    }
    if snapshot.terminal.is_some() {
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        authorize()?;
        match subscription.pop().map_err(|_| Ended::Closed)? {
            Some(event) if event.kind == EventKind::Terminal => {
                let terminal = status.read(uid, operation).map_err(|_| Ended::Closed)?;
                send_event(connection, operation, event, &terminal)?;
                return Ok(());
            }
            Some(event) => send_event(connection, operation, event, &snapshot)?,
            None if registry.strong_count() == 0 => return Err(Ended::Closed),
            None => wait_for_event(connection)?,
        }
    }
    Err(Ended::Closed)
}

fn wait_for_event(connection: &Connection) -> Result<(), Ended> {
    let mut fd = libc::pollfd {
        fd: connection.fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: fd is valid and the poll structure remains writable during the call.
    let result = unsafe { libc::poll(&mut fd, 1, 20) };
    if result == 0
        || result < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
    {
        Ok(())
    } else {
        Err(Ended::Closed)
    }
}

fn send_event(
    connection: &Connection,
    operation: OperationId,
    event: Event,
    snapshot: &OperationSnapshot,
) -> Result<(), Ended> {
    let frame = if event.kind == EventKind::Terminal {
        let terminal = snapshot.terminal.clone().ok_or(Ended::Closed)?;
        event_frame(
            operation,
            event.sequence,
            wire::OperationEvent {
                feedback: None,
                kind: wire::EventKind::EventTerminal as i32,
                phase: wire::OperationPhase::OpTerminal as i32,
                terminal: Some(terminal),
                ..Default::default()
            },
        )
    } else {
        crate::answer::operation_progress_frame(
            operation,
            &event,
            connection.session.capture_feedback_v1(),
        )
        .ok_or(Ended::Closed)?
    };
    send(connection, frame)
}

fn gap_frame(operation: OperationId, after: u64, snapshot: OperationSnapshot) -> ClientFrame {
    event_frame(
        operation,
        snapshot.last_event_sequence,
        wire::OperationEvent {
            feedback: None,
            kind: wire::EventKind::EventGap as i32,
            phase: snapshot.phase,
            gap_after_sequence: after,
            snapshot: Some(snapshot),
            ..Default::default()
        },
    )
}

fn event_frame(
    operation: OperationId,
    sequence: u64,
    mut event: wire::OperationEvent,
) -> ClientFrame {
    event.operation_id = operation.as_bytes().to_vec();
    event.sequence = sequence;
    ClientFrame {
        operation_id: event.operation_id.clone(),
        event_sequence: sequence,
        payload: Some(Payload::OperationEvent(event)),
        ..Default::default()
    }
}

fn send_snapshot(
    connection: &Connection,
    request_id: &[u8],
    snapshot: OperationSnapshot,
) -> Result<(), Ended> {
    send(
        connection,
        ClientFrame {
            request_id: request_id.to_vec(),
            payload: Some(Payload::OperationSnapshot(snapshot)),
            ..Default::default()
        },
    )
}

fn authority_error(error: AuthorityRefusal) -> ErrorCode {
    match error {
        AuthorityRefusal::Denied => ErrorCode::ErrorUnauthorized,
        _ => ErrorCode::ErrorUnavailable,
    }
}

fn refuse(connection: &Connection, request_id: &[u8], code: ErrorCode) -> Result<(), Ended> {
    send(
        connection,
        error_frame(request_id, code, "operation observation refused"),
    )
}

fn send(connection: &Connection, mut frame: ClientFrame) -> Result<(), Ended> {
    let version = connection
        .session
        .negotiated()
        .ok_or_else(|| Ended::Refused("version unavailable".into()))?;
    frame.major = version.major;
    frame.minor = version.minor;
    write_frame(connection, &frame.encode_to_vec()).map_err(Ended::Broken)
}

#[cfg(test)]
#[path = "observation_stream.tests.rs"]
mod tests;
