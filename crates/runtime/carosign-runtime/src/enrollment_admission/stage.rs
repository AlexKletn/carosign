#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    Ready,
    /// The command could start: repetition is forbidden even in case of panic of the performer.
    Uncertain,
    Finished,
    Revoked,
}
