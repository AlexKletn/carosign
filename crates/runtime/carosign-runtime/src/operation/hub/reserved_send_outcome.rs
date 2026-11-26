/// The result of one call to the record; the unknown outcome is not repeated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservedSendOutcome {
    NotSent,
    Sent,
    Unknown,
}
