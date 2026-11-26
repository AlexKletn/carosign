//! Linking to `libpam`.
//!
//! This module isolates unsafe PAM FFI from the domain, where `forbid(unsafe_code)` still
//! applies.
//!
//! The module is assembled in `cdylib`But its installation and configuration of the PAM stack
//! remains with the system owner.

#![allow(dead_code)]

mod authentication;
mod entrypoints;
mod localization;

pub(crate) use authentication::SUDO_MAX_ATTEMPTS;
pub use entrypoints::{authenticate_outcome, pam_sm_authenticate, pam_sm_setcred};
