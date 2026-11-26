#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum HelloStage {
    Prepared,
    Offered,
    Closed,
}
