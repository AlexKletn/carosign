/// What kind of machine does the phase make sense?
///
/// ⚠️ It is necessary that the phase of someone else's car does not hang on the condition:
/// the check cannot be in `Arming`and the change in `EvaluatingEvidence`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PhaseFamily {
    /// Testing.
    Verification,
    /// Change (registration and deletion).
    Mutation,
    /// There are both machines (or the operation at all).
    Shared,
}
