use crate::{model::MutationRecord, store::DurableReceipt};

/// Prepared change: the disc was intentional.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PreparedMutation {
    /// Recording of the journal (in phase) `Prepared`).
    pub record: MutationRecord,
    /// Confirmation tape.
    pub receipt: DurableReceipt,
}
