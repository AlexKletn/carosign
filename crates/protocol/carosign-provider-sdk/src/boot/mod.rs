//! Starting an worker: what to check before talking.
//!
//! ⚠️ The worker does not open the hardware until `BindAuthenticator` (Document 06): an
//! broker allows access, but no access is available until the link is made.
//!
//! ⚠️ And it doesn't write anything in the platform's vault: every record goes through the
//! platform, not past the log.

mod boot_error;
mod worker_boot;

pub use boot_error::BootError;
pub use worker_boot::Boot;
