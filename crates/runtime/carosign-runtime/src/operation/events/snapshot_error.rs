/// Why can't a picture be made from an actor's state?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapshotError {
    /// The condition refers to another operation.
    WrongOperation,
    /// A complete change requires a sustainable source of outcome.
    MutationOutcomeNeedsStorage,
    /// The terminal state of the check is incomplete or not yet fixed.
    IncompleteVerification,
}
