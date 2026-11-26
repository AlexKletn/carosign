use crate::store::DurableReceipt;

/// The result of the fixation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RegistryCommit {
    /// Confirmation tape.
    pub receipt: DurableReceipt,
}
