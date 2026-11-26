/// Source of regular result; not restored after session loss.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LiveMutationConfirmation {
    TrustedEnrollmentRollback,
    /// The software result of the authorized provider, verified by the composition.
    TrustedProvider,
}
