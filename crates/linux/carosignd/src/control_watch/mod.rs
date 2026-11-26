//! Limited internal owner of recall signals and term.
//!
//! The channel itself is not a trusted source: the effects are held until explicitly
//! transmitted to the performer; stops and ruptures of the source quell the verification.

mod control_claim;
mod control_owner;
mod revocation_sender;
mod revocation_signal;
mod signal_error;
mod slot;
mod start_error;
mod watch_handle;
mod watch_result;

pub(crate) use control_claim::ControlClaim;
pub(crate) use control_owner::ControlOwner;
pub(crate) use revocation_sender::RevocationSender;
pub(crate) use signal_error::SignalError;
pub(crate) use start_error::StartError;
pub(crate) use watch_handle::WatchHandle;
pub(crate) use watch_result::WatchResult;
