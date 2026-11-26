//! Customer service: one request, one answer.
//!
//! ⚠️ The shell doesn't store the status of the service, it doesn't "guess" the answer. The
//! answer that didn't come is an error, not a blank list.

mod client_error;
mod is_read_only;
mod reply;

pub use client_error::ClientError;
pub use is_read_only::is_read_only;
pub use reply::Reply;
