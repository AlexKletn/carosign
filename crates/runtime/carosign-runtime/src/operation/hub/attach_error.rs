/// Why not connect the observer to the current stream?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttachError {
    /// That number's already taken.
    Duplicate,
    /// The limit of the observers of the operation has been reached.
    Limit,
    /// Before connecting, you need to repeat the story or get a picture.
    CatchupRequired,
    /// The actor belongs to another operation.
    WrongOperation,
    /// Subscription copy count is exhausted.
    GenerationExhausted,
}
