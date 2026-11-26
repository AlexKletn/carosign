#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VerificationDispatchStage {
    NotStarted,
    Sending,
    Sent,
    NotSent,
    OutcomeUnknown,
    Delivered,
}
