#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotError {
    Unavailable,
    WrongReference,
    Expired,
    Revoked,
    InUse,
    /// The monotonous generation is exhausted.
    GenerationExhausted,
}
