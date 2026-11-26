use carosign_protocol::carosign::{
    broker_relay::v1::{Event, frame::Body},
    provider::v1::provider_frame::Payload,
};

use super::session_state::SessionState;

pub(super) fn check_provider_event(state: &mut SessionState, raw: &[u8]) -> Result<Vec<u8>, ()> {
    let frame = state.inbound.accept(raw).map_err(|_| ())?;
    if !matches!(frame.payload, Some(Payload::WorkerHello(_)))
        && frame.worker_instance_id != state.context.worker
    {
        return Err(());
    }
    if !matches!(
        frame.payload,
        Some(Payload::WorkerHello(_) | Payload::Initialized(_))
    ) && frame.device_epoch != state.context.epoch
    {
        return Err(());
    }
    match frame.payload.as_ref().ok_or(())? {
        Payload::WorkerHello(hello) => state.setup.advertise(hello),
        Payload::Initialized(value) => state.setup.confirm_initialize(&frame, value)?,
        Payload::AuthenticatorReady(value) => {
            state
                .setup
                .confirm_ready(state.context.epoch, state.scope.as_ref(), value)?;
        }
        Payload::PrebindDescription(value) if state.scope.is_none() => {
            state.setup.accept_description(&frame, value)?;
        }
        Payload::CommandAck(ack)
            if state.active_operation.is_none() && state.challenge_preparation.is_some() =>
        {
            state
                .challenge_preparation
                .as_mut()
                .ok_or(())?
                .acknowledge(&frame, ack, state.scope.as_ref())?;
        }
        Payload::EnrollmentTransactionResult(result) => {
            state.active_operation.as_mut().ok_or(())?.finish_rollback(
                state.scope.as_ref(),
                &frame,
                result,
            )?;
        }
        payload @ (Payload::CommandAck(_)
        | Payload::ProviderProgress(_)
        | Payload::CancelReceipt(_)
        | Payload::VerificationOutcome(_)
        | Payload::MutationOutcome(_)) => {
            state.active_operation.as_ref().ok_or(())?.check_event(
                state.scope.as_ref(),
                &frame,
                payload,
            )?;
        }
        _ => return Err(()),
    }
    let packet = state.outbound_frame(Body::Event(Event { frame: Some(frame) }));
    state.advance_outbound()?;
    Ok(packet)
}
