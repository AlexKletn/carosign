//! Owner of the operation: communication with the caller and observation of the recall.
//!
//! An owner reference conveys no permission to obtain an operation.
//! It means only who to deliver the result and whose disabling quells the success (document
//! 22, rule 7).
//!
//! ⚠️ Disabling the owner for verification is a reason to suppress the result; for a change,
//! it is not a reason to lose the reconciliation obligation.

mod owner_channel;
mod owner_handle;

pub use owner_channel::OwnerChannel;
pub use owner_handle::OwnerHandle;
