//! Completion of W07: Definition to acceptance in the scope of document 16.
//!
//! ⚠️ What is left of document 16 for W07:
//!
//! * `carosign-linux/src/inventory` - the actual inventory of devices (not
//! A photo and collection of signs from the system;
//! * `carosign-linux/src/process/*` - verified launch on `sys`not
//! It's just rules.
//!
//! ⚠️ The W07 rule in its entirety (Document 12): **zero installed providers is healthy** A
//! platform without providers simply can't print, and that's no reason to declare service
//! broken.

pub(crate) mod archive;
pub(crate) mod installed;
pub mod inventory;
pub mod launch;
pub(crate) mod policy_file;
pub mod publication_owner;
pub(crate) mod signature_inputs;
pub(crate) mod signature_policy;
pub(crate) mod signature_runtime;
pub(crate) mod signature_status;
pub(crate) mod supervisor_policy;
pub(crate) mod verified_installation;
pub(crate) mod verity;

pub use inventory::{DeviceEntry, InventoryScan, ScanError};
pub use launch::{LaunchPlan, LaunchRefusal, PreparedLaunch};
