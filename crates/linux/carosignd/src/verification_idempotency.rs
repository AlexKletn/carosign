//! Canonical hash of test parameters to account for repetitions.
//!
//! It's not an authority, it's not a repository of operations. It's the policy source's job
//! to check the caller and the permission before comparing hashes.

use carosign_core::authorization::GrantScope;
use carosign_core::operation::context::VerificationPurpose;
use carosign_runtime::authority::verification::{
    PurposeHint, RequestedSubject, VerificationIntent,
};

fn put_field(out: &mut Vec<u8>, field: &[u8]) -> Result<(), InvalidScope> {
    let len = u32::try_from(field.len()).map_err(|_| InvalidScope)?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(field);
    Ok(())
}

#[path = "verification_idempotency.tests.rs"]
#[cfg(test)]
mod tests;

mod request_digest;
pub(crate) use request_digest::RequestDigest;
mod invalid_scope;
pub(crate) use invalid_scope::InvalidScope;
