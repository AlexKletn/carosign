/// Why a sustainable result cannot be published to the owner
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerminalPublicationError {
    /// The status of the transaction or the permit does not match the request.
    ContextMismatch,
    /// A sustainable outcome has not been confirmed.
    StorageUnavailable,
    /// The stream was unable to accept the terminal event.
    Event(carosign_runtime::operation::hub::TerminalPublishError),
}
