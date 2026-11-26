//! Provides the fprintd API on top of the biometric platform.
//!
//! The adapter neither owns the scanner nor exposes templates. See specification 07.

pub mod dbus;
mod diagnostics;
mod fingerprint_label;
pub mod ownership;
pub mod sender;
mod system_backend;
pub mod translate;
