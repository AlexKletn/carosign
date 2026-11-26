/// Refusal to publish does not release a withheld step.
#[derive(Debug)]
pub(crate) enum StepPublishError {
    Missing,
    WrongOperation,
    TerminalRequiresAuthorization,
    Event(carosign_runtime::operation::events::EventError),
    /// There is no room for notifications about disabled observers.
    NotificationLimit,
}
