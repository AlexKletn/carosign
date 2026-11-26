//! Verification of the machine report of a separate installer verifying the signature.
//!
//! The report and the output code are only valid from a validation process run by a trusted
//! installer over the fixed data and a trusted set of keys. Line parsing does not perform
//! cryptographic verification on its own.
#![allow(dead_code)]

mod cap_data;
mod cap_header;
mod check;
#[cfg(test)]
mod gpg_home;
mod reap;
mod signature_refusal;
mod verification_error;
mod verify_in_isolated_root;
#[cfg(test)]
mod verify_in_single_threaded_helper;
mod wait_and_check;

pub(super) use check::check;
use check::fingerprint;
pub(super) use signature_refusal::SignatureRefusal;
pub(super) use verification_error::VerificationError;
pub(super) use verify_in_isolated_root::verify_in_isolated_root;
#[cfg(test)]
pub(super) use verify_in_single_threaded_helper::verify_in_single_threaded_helper;
use wait_and_check::wait_and_check;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
