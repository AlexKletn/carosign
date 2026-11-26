/// What to do with the recording at launch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecoveryDecision {
    /// The only intention was to close as “nothing has changed.”
    CloseAsNoChange,
    /// The hardware could have changed: the area was blocked and checked.
    BlockAndReconcile,
    /// Recording in an incomprehensible phase: refusal, understood by a person.
    NeedsDecision,
    /// The recording is closed and we believe in closing: nothing needs to be done.
    Closed,
}
