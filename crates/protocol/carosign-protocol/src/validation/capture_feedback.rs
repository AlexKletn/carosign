use crate::carosign::common::v1::{
    CapabilityDescriptor, CaptureAction as Action, CaptureEventKind, CaptureFeedback,
    CapturePrompt, CaptureRetryReason as Reason, CaptureState, CaptureTarget as Target, Modality,
    capture_feedback::Kind,
};

pub fn check_capture_feedback(feedback: &CaptureFeedback) -> bool {
    match feedback.kind {
        Some(Kind::Prompt(prompt)) => valid_prompt(prompt),
        Some(Kind::CaptureRetry(retry)) => {
            retry.reason.is_none_or(|reason| {
                Reason::from_i32(reason).is_some() && reason != Reason::Unspecified as i32
            }) && retry.prompt.is_none_or(valid_prompt)
        }
        Some(Kind::Percent(percent)) => percent <= 100,
        Some(Kind::AcceptedSamples(samples)) => {
            samples.required > 0 && samples.accepted <= samples.required
        }
        None => false,
    }
}

fn valid_prompt(prompt: CapturePrompt) -> bool {
    use Action::*;
    use Target::*;
    matches!(
        (
            Action::from_i32(prompt.action),
            Target::from_i32(prompt.target)
        ),
        (Some(Place | Center | Move | Remove | Swipe), Some(Finger))
            | (Some(Center | HoldStill), Some(Face))
            | (Some(LookAt), Some(Camera))
            | (Some(ImproveLighting), Some(Scene))
            | (Some(Align | HoldStill), Some(Eye))
            | (Some(Speak | SpeakLouder), Some(Voice))
            | (Some(ReduceNoise), Some(Environment))
    )
}

pub fn capture_feedback_is_declared(
    feedback: &CaptureFeedback,
    device: &CapabilityDescriptor,
) -> bool {
    let Some(modality) = Modality::from_i32(device.modality) else {
        return false;
    };
    let swipe = device
        .interaction_modes
        .contains(&(crate::carosign::common::v1::InteractionMode::InteractionSwipe as i32));
    let kind = match feedback.kind {
        Some(Kind::Prompt(_)) => CaptureEventKind::CaptureEventPrompt,
        Some(Kind::CaptureRetry(_)) => CaptureEventKind::CaptureEventRetry,
        Some(Kind::Percent(_) | Kind::AcceptedSamples(_)) => CaptureEventKind::CaptureEventProgress,
        None => return false,
    };
    device.supported_events.contains(&(kind as i32))
        && capture_feedback_matches_device(feedback, modality, swipe)
}

fn capture_feedback_matches_device(
    feedback: &CaptureFeedback,
    modality: Modality,
    swipe: bool,
) -> bool {
    if !check_capture_feedback(feedback) || modality == Modality::Unspecified {
        return false;
    }
    let prompt_matches = |prompt: CapturePrompt| match Target::from_i32(prompt.target) {
        Some(Target::Finger) => {
            modality == Modality::Fingerprint && (prompt.action != Action::Swipe as i32 || swipe)
        }
        Some(Target::Face) => modality == Modality::Face,
        Some(Target::Eye) => modality == Modality::Iris,
        Some(Target::Camera | Target::Scene) => matches!(modality, Modality::Face | Modality::Iris),
        Some(Target::Voice | Target::Environment) => modality == Modality::Voice,
        _ => false,
    };
    match feedback.kind {
        Some(Kind::Prompt(prompt)) => prompt_matches(prompt),
        Some(Kind::CaptureRetry(retry)) => {
            let reason_matches = match retry.reason.and_then(Reason::from_i32) {
                None | Some(Reason::PoorQuality | Reason::TooFast) => true,
                Some(Reason::PartialCapture) => modality == Modality::Fingerprint,
                Some(Reason::SwipeTooShort) => modality == Modality::Fingerprint && swipe,
                Some(Reason::PoorLighting | Reason::OutOfFrame | Reason::Motion) => {
                    matches!(modality, Modality::Face | Modality::Iris)
                }
                Some(Reason::TooQuiet | Reason::BackgroundNoise) => modality == Modality::Voice,
                Some(Reason::Unspecified) => false,
            };
            reason_matches && retry.prompt.is_none_or(prompt_matches)
        }
        Some(Kind::Percent(_) | Kind::AcceptedSamples(_)) => true,
        None => false,
    }
}

pub fn valid_capture_state(state: &CaptureState) -> bool {
    Target::from_i32(state.target).is_some_and(|target| target != Target::Unspecified)
        && (state.present.is_some() || state.needed.is_some())
}

pub fn capture_state_is_declared(state: &CaptureState, device: &CapabilityDescriptor) -> bool {
    use Modality::*;
    valid_capture_state(state)
        && device
            .supported_events
            .contains(&(CaptureEventKind::CaptureEventState as i32))
        && matches!(
            (state.target(), device.modality()),
            (Target::Finger, Fingerprint)
                | (Target::Face, Face)
                | (Target::Eye, Iris)
                | (Target::Voice, Voice)
        )
}

#[path = "capture_feedback.tests.rs"]
#[cfg(test)]
mod tests;
