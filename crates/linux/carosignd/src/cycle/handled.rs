/// What to do after the shot.
#[derive(Clone, PartialEq, Debug)]
pub enum Handled {
    /// We do not respond (salutations are given separately).
    Silent,
    /// Respond with this shot.
    Answer(Vec<u8>),
    /// The start of the operation is accepted; the result is only after completion.
    Started {
        /// Acceptance frame.
        accepted: Vec<u8>,
        /// A frame of the confirmed result if the operation is completed.
        terminal: Option<Vec<u8>>,
    },
    /// Refusal.
    Refused(Vec<u8>),
}
