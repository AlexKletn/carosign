#[derive(Clone, Copy)]
pub(super) enum MockReply {
    Same,
    Foreign,
    ForeignWorker,
    ForeignEpoch,
    ForeignResources,
    ForeignMode,
    Unavailable,
    Pending,
}
