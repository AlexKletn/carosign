//! PAM: **client only.** Sensor doesn't touch, hardware doesn't open directly.
//!
//! ⚠️ The rule for which the adapter exists separately (document 07): `PAM_USER` - not an
//! authority, `AcceptedFactor` - not logging in.
//! Success returns only when the request and the policy are met.
//!
//! Adapter installation leaves `/etc/pam.d` untouched. The administrator applies the
//! documented examples and recovery procedure.

pub mod client;
pub mod conversation;
pub mod pam_ffi;
pub mod result;
mod tty_ui;

pub use result::{Outcome, PamCode, to_code};
