use super::EventNotification;

/// Context of publishing events belonging to one operation.
pub struct ObservedExecution<'a> {
    /// The hub of this operation.
    pub hub: &'a mut carosign_runtime::operation::hub::EventHub,
    /// Non-blocking transmission of connection notifications.
    pub notify: &'a mut EventNotification<'a>,
    /// Read a stable terminal outcome if it is connected.
    pub terminal_storage:
        Option<&'a dyn carosign_runtime::operation::observer::TerminalOutcomePort>,
}
