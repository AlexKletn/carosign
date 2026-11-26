use carosign_protocol::carosign::{client::v1::OperationEvent, common::v1::capture_feedback::Kind};

use super::capture_feedback::retry_status;
use crate::dbus::EnrollProgress;

pub(super) const MAX_STAGES: u32 = 128;

#[derive(Default)]
pub(super) struct EnrollmentProgress {
    stages: Option<u32>,
    reported: u32,
}

impl EnrollmentProgress {
    pub(super) fn observe(&mut self, event: &OperationEvent, mut emit: impl FnMut(EnrollProgress)) {
        if let Some(Kind::AcceptedSamples(samples)) = event.feedback.and_then(|value| value.kind) {
            if !(1..=MAX_STAGES).contains(&samples.required) || samples.accepted > samples.required
            {
                return;
            }
            let stages = *self.stages.get_or_insert_with(|| {
                emit(EnrollProgress::Stages(samples.required));
                samples.required
            });
            if samples.required != stages {
                return;
            }
            let intermediate = samples.accepted.min(stages - 1);
            for _ in self.reported..intermediate {
                emit(EnrollProgress::StagePassed);
            }
            self.reported = self.reported.max(intermediate);
        } else if let Some(status) = retry_status(event) {
            emit(EnrollProgress::Retry(status));
        }
    }
}

#[path = "enrollment_progress.tests.rs"]
#[cfg(test)]
mod tests;
