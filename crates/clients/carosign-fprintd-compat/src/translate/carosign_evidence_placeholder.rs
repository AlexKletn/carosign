#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutcomeKind {
    Accepted,
    NotMatched,
    Cancelled,
    Unavailable,
    Failed,
}
