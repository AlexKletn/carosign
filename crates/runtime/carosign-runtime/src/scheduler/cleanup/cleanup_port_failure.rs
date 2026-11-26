/// Uncertain outcome of a trusted broker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupPortFailure {
    Unavailable,
    Uncertain,
}
