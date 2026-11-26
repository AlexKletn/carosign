use carosign_protocol::carosign::{
    broker_relay::v1::{Frame, control, frame::Body},
    provider::v1::{ProviderFrame, RequestCancel, provider_frame::Payload},
};
use prost::Message;

use crate::{relay::request_id, relay_pump::CheckedRequest};

use super::{
    active_operation::ActiveOperation,
    challenge_preparation::ChallengePreparation,
    owner_revocation::{await_owner_revoke, revoke_owner},
    reply::Reply,
    request_validation::{valid_id, valid_mutation},
    resource_binding::ResourceBinding,
    session_state::SessionState,
};

pub(super) fn check_request(
    state: &mut SessionState,
    resource_binding: &mut ResourceBinding,
    pending_events: bool,
    raw: &[u8],
) -> Result<CheckedRequest, ()> {
    let frame = Frame::decode(raw).map_err(|_| ())?;
    if frame.encode_to_vec() != raw
        || frame.major != 1
        || frame.minor != 0
        || frame.seq != state.inbound_seq
        || frame.worker_instance != state.context.worker
        || frame.device_epoch != state.context.epoch
    {
        return Err(());
    }
    let expected = request_id(state.context.broker, state.inbound_seq).map_err(|_| ())?;
    if state.lease_revoked {
        return Err(());
    }
    if let Some(Body::Control(control)) = frame.body.as_ref()
        && control.request_id == expected
        && let Some(control::Action::Revoke(revoke)) = control.action.as_ref()
    {
        revoke_owner(
            state.scope.as_ref(),
            revoke.scope.as_ref(),
            !state.replies.is_empty(),
            pending_events,
            || await_owner_revoke(resource_binding.owner_session.as_mut().ok_or(())?, expected),
        )?;
        state.inbound_seq = state.inbound_seq.checked_add(1).ok_or(())?;
        state.lease_revoked = true;
        return Ok(CheckedRequest::Local {
            request_id: expected,
        });
    }
    let mut bind_fds = Vec::new();
    let (reply, mut provider) = match frame.body.ok_or(())? {
        Body::Command(command) if command.request_id == expected => {
            let provider = command.frame.ok_or(())?;
            match provider.payload.as_ref().ok_or(())? {
                Payload::Initialize(init)
                    if command.mutation_permit.is_none() && state.scope.is_none() =>
                {
                    state.setup.begin_initialize(init)?;
                }
                Payload::PrebindDescribe(request)
                    if command.mutation_permit.is_none()
                        && resource_binding.allows_prebind()
                        && state.scope.is_none() =>
                {
                    state
                        .setup
                        .request_description(state.context.epoch, &provider, request)?;
                }
                Payload::Shutdown(_) if command.mutation_permit.is_none() => {}
                Payload::StartVerification(verification)
                    if command.mutation_permit.is_none()
                        && state.setup.is_ready()
                        && state.active_operation.is_none()
                        && state.challenge_preparation.is_none() =>
                {
                    let owner = state
                        .scope
                        .as_ref()
                        .and_then(|s| s.owner.as_ref())
                        .ok_or(())?;
                    let authenticator = verification.authenticator.as_ref().ok_or(())?;
                    if provider.operation_id != owner.operation_id
                        || authenticator.authenticator_id != owner.authenticator_id
                        || authenticator.device_epoch != state.context.epoch
                    {
                        return Err(());
                    }
                    state.active_operation =
                        Some(ActiveOperation::verification(&provider.command_id));
                }
                Payload::PrepareEnrollmentChallenge(value)
                    if command.mutation_permit.is_none()
                        && state.setup.is_ready()
                        && state.active_operation.is_none()
                        && state.challenge_preparation.is_none() =>
                {
                    let held = state.scope.as_ref().ok_or(())?;
                    state.challenge_preparation = Some(ChallengePreparation::begin(
                        &state.context,
                        held,
                        &provider,
                        value,
                    )?);
                }
                Payload::StartEnrollment(value)
                    if state.setup.is_ready()
                        && state.active_operation.is_none()
                        && state.challenge_preparation.is_none() =>
                {
                    let given = state.scope.as_ref().ok_or(())?;
                    let permit = command.mutation_permit.as_ref().ok_or(())?;
                    if !valid_mutation(
                        &state.context,
                        given,
                        permit,
                        &provider,
                        &value.mutation_id,
                        value.subject.as_ref(),
                        value.authenticator.as_ref(),
                    ) {
                        return Err(());
                    }
                    state.active_operation = Some(ActiveOperation::enrollment(
                        &provider.command_id,
                        &value.mutation_id,
                    ));
                }
                Payload::ContinueEnrollment(value)
                    if state.setup.is_ready() && state.active_operation.is_none() =>
                {
                    let value = value.enrollment.as_ref().ok_or(())?;
                    if !state
                        .challenge_preparation
                        .as_ref()
                        .ok_or(())?
                        .matches_continuation(&provider, value)
                    {
                        return Err(());
                    }
                    if value.enrollment_authorization_ref.len() != 16
                        || value.enrollment_challenge_digest.len() != 32
                    {
                        return Err(());
                    }
                    let given = state.scope.as_ref().ok_or(())?;
                    let permit = command.mutation_permit.as_ref().ok_or(())?;
                    if !valid_mutation(
                        &state.context,
                        given,
                        permit,
                        &provider,
                        &value.mutation_id,
                        value.subject.as_ref(),
                        value.authenticator.as_ref(),
                    ) {
                        return Err(());
                    }
                    state.active_operation = Some(ActiveOperation::enrollment(
                        &provider.command_id,
                        &value.mutation_id,
                    ));
                }
                Payload::EnrollmentTransactionRequest(request)
                    if state.setup.transactions_enabled() && command.mutation_permit.is_none() =>
                {
                    state.active_operation.as_mut().ok_or(())?.begin_rollback(
                        state.scope.as_ref(),
                        &provider,
                        request,
                    )?;
                }
                Payload::StartRemoval(value)
                    if state.setup.is_ready()
                        && state.active_operation.is_none()
                        && state.challenge_preparation.is_none() =>
                {
                    let given = state.scope.as_ref().ok_or(())?;
                    let permit = command.mutation_permit.as_ref().ok_or(())?;
                    if !valid_mutation(
                        &state.context,
                        given,
                        permit,
                        &provider,
                        &value.mutation_id,
                        value.subject.as_ref(),
                        value.authenticator.as_ref(),
                    ) {
                        return Err(());
                    }
                    resource_binding.dispatch_to_owner(expected, &provider)?;
                    state.active_operation = Some(ActiveOperation::removal(
                        &provider.command_id,
                        &value.mutation_id,
                    ));
                }
                _ => return Err(()),
            }
            (Reply::Command, provider)
        }
        Body::Bind(bind)
            if bind.request_id == expected
                && state.setup.is_initialized()
                && state.scope.is_none() =>
        {
            let (given, provider, descriptors) =
                resource_binding.bind(&state.context, state.setup.profile(), expected, bind)?;
            bind_fds = descriptors;
            state.scope = Some(given);
            (Reply::Bind, provider)
        }
        Body::Control(control) if control.request_id == expected => {
            let Some(control::Action::Cancel(cancel)) = control.action else {
                return Err(());
            };
            let given = cancel.scope.ok_or(())?;
            let operation = state.active_operation.as_mut().ok_or(())?;
            if state.scope.as_ref() != Some(&given)
                || operation.is_cancelled()
                || !state.replies.is_empty()
                || pending_events
                || !valid_id(&cancel.cancel_command_id)
            {
                return Err(());
            }
            operation.cancel(&cancel.cancel_command_id);
            let operation = given.owner.ok_or(())?.operation_id;
            (
                Reply::Cancel,
                ProviderFrame {
                    major: 1,
                    minor: carosign_protocol::limits::MINOR_BASELINE,
                    worker_instance_id: state.context.worker.to_vec(),
                    frame_seq: 0,
                    command_id: cancel.cancel_command_id,
                    operation_id: operation.clone(),
                    device_epoch: state.context.epoch,
                    uid: None,
                    payload: Some(Payload::RequestCancel(RequestCancel {
                        target_operation_id: operation,
                    })),
                },
            )
        }
        _ => return Err(()),
    };
    if provider.worker_instance_id != state.context.worker
        || provider.frame_seq != 0
        || provider.device_epoch
            != if matches!(provider.payload, Some(Payload::Initialize(_))) {
                0
            } else {
                state.context.epoch
            }
        || provider.major != 1
        || provider.minor != carosign_protocol::limits::MINOR_BASELINE
    {
        return Err(());
    }
    provider.frame_seq = 1;
    carosign_protocol::validation::check_provider_envelope(&provider).map_err(|_| ())?;
    provider.frame_seq = 0;
    if state.replies.insert(expected, reply).is_some() {
        return Err(());
    }
    state.inbound_seq = state.inbound_seq.checked_add(1).ok_or(())?;
    let bytes = provider.encode_to_vec();
    Ok(if matches!(reply, Reply::Cancel) {
        CheckedRequest::Control {
            request_id: expected,
            bytes,
        }
    } else if matches!(reply, Reply::Bind) {
        CheckedRequest::Bind {
            request_id: expected,
            bytes,
            fds: bind_fds,
        }
    } else {
        CheckedRequest::Command {
            request_id: expected,
            bytes,
        }
    })
}
