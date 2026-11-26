/// What to do with the rendition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Handoff {
    /// Give it.
    Deliver,
    /// Refuse and close everything that came.
    RejectAndCloseAll,
}
