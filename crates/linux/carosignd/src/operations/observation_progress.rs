/// The result of the transmission of the observation of the saved operation.
#[derive(Debug)]
pub enum ObservationProgress {
    /// The actor has taken the step; the caller is obliged to process the effects issued.
    Advanced(carosign_runtime::operation::actor::Step),
    /// The actor withheld observation until proof check or replay.
    Held(carosign_runtime::operation::actor::ObservationError),
}
