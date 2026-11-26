//! Run an worker according to the W07 rules on real kernel calls.
//!
//! An open object is fixed without clicking on symbolic references; its metadata is read
//! through a descriptor, not by re-defining the path.
//! The descriptor itself does not confirm the origin or immutability of the content.
//!
//! ⚠️ It's just a launch prep. `exec` It makes the root of the assembly, because after `exec`
//! It's a code that doesn't exist.

mod launch_plan;
mod launch_refusal;
mod prepared_installation;
mod prepared_launch;

pub use launch_plan::LaunchPlan;
pub use launch_refusal::LaunchRefusal;
pub use prepared_installation::{PreparedInstallation, prepare_installed};
#[cfg(test)]
use prepared_launch::root_owned_protected;
pub use prepared_launch::{PreparedLaunch, prepare};

pub use super::verified_installation::{
    InstallationMetadataDecoder, MetadataDecodeError, ParsedInstallationManifest,
};

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
