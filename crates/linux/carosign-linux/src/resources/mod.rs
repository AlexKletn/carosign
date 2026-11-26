//! Linux resources: descriptors, rights, issuance.
//!
//! ⚠️ The given descriptor is not subject to recall - only the completion of the process.
//! The rule lives in the`fd`] and not in the comments to the broker code.

pub mod broker_client;
pub mod claim;
pub mod fd;
