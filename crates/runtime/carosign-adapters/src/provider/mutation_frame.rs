use super::{MutationFrameOnly, ProviderBridge, ProviderFailure};
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
use carosign_protocol::carosign::{common::v1 as common, provider::v1 as provider};

/// It builds a full frame out of an unchanging plan, and the frame itself doesn't allow
/// sending.
pub fn mutation_frame(
    context: &carosign_runtime::ports::MutationArmRequest,
) -> Result<provider::ProviderFrame, ProviderFailure> {
    if !context.is_consistent() {
        return Err(ProviderFailure::Rejected);
    }
    let mut frame = ProviderBridge::<MutationFrameOnly>::frame(
        *context.arm.worker.as_bytes(),
        context.arm.epoch.value(),
        context.operation,
        &context.plan,
    );
    frame.uid = Some(context.uid);
    let subject = context.logical_plan.subject();
    let wire_subject = common::SubjectRef {
        authority_id: subject.authority.as_bytes().to_vec(),
        subject_id: subject.subject.as_bytes().to_vec(),
    };
    let authenticator = common::AuthenticatorRef {
        authenticator_id: context.logical_plan.authenticator().as_bytes().to_vec(),
        device_epoch: context.arm.epoch.value(),
    };
    match frame.payload.as_mut() {
        Some(Payload::StartEnrollment(value)) => {
            value.subject = Some(wire_subject);
            value.authenticator = Some(authenticator);
            value.label = context
                .label
                .as_ref()
                .ok_or(ProviderFailure::Rejected)?
                .as_str()
                .to_owned();
        }
        Some(Payload::StartRemoval(value)) => {
            value.subject = Some(wire_subject);
            value.authenticator = Some(authenticator);
        }
        _ => return Err(ProviderFailure::Rejected),
    }
    Ok(frame)
}

pub fn continuation_frame(
    context: &carosign_runtime::ports::MutationArmRequest,
) -> Result<provider::ProviderFrame, ProviderFailure> {
    let mut frame = mutation_frame(context)?;
    let Some(Payload::StartEnrollment(enrollment)) = frame.payload.take() else {
        return Err(ProviderFailure::Rejected);
    };
    if enrollment.enrollment_authorization_ref.is_empty()
        || enrollment.enrollment_challenge_digest.len() != 32
    {
        return Err(ProviderFailure::Rejected);
    }
    frame.payload = Some(Payload::ContinueEnrollment(provider::ContinueEnrollment {
        enrollment: Some(enrollment),
    }));
    Ok(frame)
}
