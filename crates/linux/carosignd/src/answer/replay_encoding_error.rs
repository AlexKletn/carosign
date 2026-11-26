/// Why Authorized Repeats Cannot Be Converted into Frames
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReplayEncodingError {
    /// The internal history is contradictory.
    InvalidHistory,
    /// To fail, you need a separate procedure for issuing a number and a picture.
    GapNeedsSequenceContract,
}
