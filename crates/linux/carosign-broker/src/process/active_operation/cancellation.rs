pub(super) enum Cancellation {
    None,
    Requested(Vec<u8>),
    RollingBack(Vec<u8>),
    RollbackResultReceived,
}
