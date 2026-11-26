//! Immutable snapshots of individual installer signature verification inputs.
//!
//! The signature is checked over the image, not the file being modified. The approved
//! measurement of the keyset provides an external system source; this module does not
//! establish the origin of the source, gpgv or its libraries.
#![allow(dead_code)]

mod input_error;
mod mount_attr;
mod pinned_verifier;
mod primary_version;
mod signature_checked_artifact;
mod signature_inputs;

pub(super) use input_error::InputError;
pub(super) use pinned_verifier::PinnedVerifier;
#[cfg(test)]
use primary_version::primary_version;
pub(super) use signature_checked_artifact::SignatureCheckedArtifact;
pub(super) use signature_inputs::SignatureInputs;
#[cfg(test)]
use signature_inputs::snapshot;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
