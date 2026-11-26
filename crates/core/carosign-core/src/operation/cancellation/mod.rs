//! Cancellation: Request and receipt are two different things.
//!
//! ⚠️ Receipt `CancelRequested` The second one comes separately, and the lease is not
//! vacated, and the transaction is not considered complete (C-04/P-08).
//!
//! The sender of the cancellation is retained: the cancellation by the owner and the
//! cancellation by the system are not the same for further proceedings.

mod cancel_receipt;
mod cancel_request;
mod cancel_source;

pub use cancel_receipt::CancelReceipt;
pub use cancel_request::CancelRequest;
pub use cancel_source::CancelSource;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
