use super::fmt;

/// Confirmation that what is recorded is actually on the disk.
///
/// ⚠️ Separate type on purpose: `Ok(())` You can't confuse a buffered record with a confirmed
/// record. `DurableReceipt` You can't do it outside of the crate, just the back of the vault,
/// which knows what it's doing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DurableReceipt {
    durable: (),
}

impl DurableReceipt {
    /// Only the backend can issue this receipt once the write is durable.
    #[allow(dead_code)] // The real back will appear with the redb.
    pub(crate) fn confirmed_by_backend() -> Self {
        Self { durable: () }
    }
}

impl fmt::Debug for DurableReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DurableReceipt(подтверждено)")
    }
}
