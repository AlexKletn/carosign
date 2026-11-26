#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum OwnerSendProgress {
    Sent,
    WouldBlock,
    Advanced,
}
