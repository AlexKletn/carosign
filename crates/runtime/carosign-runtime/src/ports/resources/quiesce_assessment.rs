/// How the cleanup ended.
///
/// ⚠️ `Uncertain` The device can work, and it can't be touched by the new owner.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QuiesceAssessment {
    /// Calm and tested.
    Quiesced,
    /// Status unknown.
    Uncertain,
}
