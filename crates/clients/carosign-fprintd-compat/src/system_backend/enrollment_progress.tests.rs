use super::*;
use carosign_protocol::carosign::common::v1::{AcceptedSamples, CaptureFeedback};

#[test]
fn counts_jumps_once_and_leaves_last_stage_to_terminal_result() {
    let mut progress = EnrollmentProgress::default();
    let mut statuses = Vec::new();
    for (accepted, required) in [(0, 4), (2, 4), (1, 4), (2, 4), (4, 5), (4, 4)] {
        let event = OperationEvent {
            feedback: Some(CaptureFeedback {
                kind: Some(Kind::AcceptedSamples(AcceptedSamples {
                    accepted,
                    required,
                })),
            }),
            ..Default::default()
        };
        progress.observe(&event, |status| statuses.push(status));
    }
    assert_eq!(
        statuses,
        [
            EnrollProgress::Stages(4),
            EnrollProgress::StagePassed,
            EnrollProgress::StagePassed,
            EnrollProgress::StagePassed,
        ]
    );
}
