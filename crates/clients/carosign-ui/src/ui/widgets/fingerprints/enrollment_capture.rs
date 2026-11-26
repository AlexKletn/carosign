use carosign_protocol::carosign::{
    client::v1::{EventKind, OperationEvent, OperationPhase},
    common::v1::{
        CaptureAction, CapturePrompt, CaptureRetryReason, CaptureTarget, capture_feedback::Kind,
    },
};

pub(super) struct EnrollmentCapture {
    pub(super) accepted: u32,
    pub(super) required: u32,
    pub(super) percent: f32,
    pub(super) message: &'static str,
    pub(super) retry: bool,
}

impl EnrollmentCapture {
    pub(super) fn new(required: u32) -> Self {
        Self {
            accepted: 0,
            required,
            percent: 0.0,
            message: "ui-enroll-event-place",
            retry: false,
        }
    }

    pub(super) fn apply(&mut self, event: &OperationEvent) {
        if let Some(feedback) = &event.feedback {
            match &feedback.kind {
                Some(Kind::AcceptedSamples(samples))
                    if samples.required > 0 && samples.accepted <= samples.required =>
                {
                    self.accepted = samples.accepted;
                    self.required = samples.required;
                    self.percent = samples.accepted as f32 * 100.0 / samples.required as f32;
                }
                Some(Kind::Percent(percent)) if *percent <= 100 => self.percent = *percent as f32,
                Some(Kind::Prompt(prompt)) => {
                    if let Some(message) = Self::prompt(prompt) {
                        self.set_message(message, false);
                    }
                }
                Some(Kind::CaptureRetry(retry)) => {
                    let message = match retry.reason.and_then(CaptureRetryReason::from_i32) {
                        Some(CaptureRetryReason::PoorQuality) => "ui-enroll-event-poor-quality",
                        Some(CaptureRetryReason::PartialCapture) => "ui-enroll-event-partial",
                        Some(CaptureRetryReason::SwipeTooShort) => "ui-enroll-event-short-swipe",
                        _ => retry
                            .prompt
                            .as_ref()
                            .and_then(Self::prompt)
                            .unwrap_or("ui-enroll-event-retry"),
                    };
                    self.set_message(message, true);
                }
                _ => {}
            }
            return;
        }

        if let Some(state) = &event.capture_state {
            if state.target() == CaptureTarget::Finger {
                match (state.present, state.needed) {
                    (Some(false), Some(true)) => self.set_message("ui-enroll-event-place", false),
                    (Some(true), Some(false)) => self.set_message("ui-enroll-event-remove", false),
                    _ => {}
                }
            }
            return;
        }

        match event.kind() {
            EventKind::EventProgress if event.percent <= 100 => self.percent = event.percent as f32,
            EventKind::EventPlaceFinger => self.set_message("ui-enroll-event-place", false),
            EventKind::EventMoveFinger => self.set_message("ui-enroll-event-move", false),
            EventKind::EventRemoveFinger => self.set_message("ui-enroll-event-remove", false),
            EventKind::EventRetry => self.set_message("ui-enroll-event-retry", true),
            EventKind::EventWorking => self.set_message("ui-enroll-event-working", false),
            EventKind::EventStateChanged if event.phase() == OperationPhase::OpFinalizing => {
                self.set_message("ui-enroll-event-finalizing", false);
            }
            _ => {}
        }
    }

    fn set_message(&mut self, message: &'static str, retry: bool) {
        self.message = message;
        self.retry = retry;
    }

    fn prompt(prompt: &CapturePrompt) -> Option<&'static str> {
        if prompt.target() != CaptureTarget::Finger {
            return None;
        }
        match prompt.action() {
            CaptureAction::Place => Some("ui-enroll-event-place"),
            CaptureAction::Center => Some("ui-enroll-event-center"),
            CaptureAction::Move => Some("ui-enroll-event-move"),
            CaptureAction::Remove => Some("ui-enroll-event-remove"),
            CaptureAction::Swipe => Some("ui-enroll-event-swipe"),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "enrollment_capture.tests.rs"]
mod tests;
