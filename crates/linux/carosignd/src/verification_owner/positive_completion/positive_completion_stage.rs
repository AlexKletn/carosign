#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PositiveCompletionStage {
    Sending,
    Receipt,
    Destroy,
    DestroyHandoff,
    Record,
    RecordHandoff,
    Publish,
    Finished,
}
