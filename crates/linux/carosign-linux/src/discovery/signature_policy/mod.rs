//! An independent, secure snapshot of the installation inspection policy.
//!
//! It's an internal preparation of the signature sources. It doesn't publish the
//! installation, it's not a launch authority. It's not a public format yet.
#![allow(dead_code)]

mod policy_error;
mod prepared_signature;
mod rules;
mod signature_policy;

pub(super) use policy_error::PolicyError;
pub(super) use prepared_signature::PreparedSignature;
use rules::Rules;
pub(super) use signature_policy::SignaturePolicy;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
