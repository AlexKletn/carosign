//! The boundary between the source of the installed package and the fixing of objects.
//!
//! Open descriptors and DAC checks do not prove the origin of the packet or the immutability
//! of its contents. Until the present adapter and active version protection mechanism, this
//! module does not produce an executable launch.

#![allow(dead_code)]

mod approved_installation;
mod approved_policy;
mod check_policy;
mod installation_authority;
mod installation_error;
mod installation_key;
mod pin_candidate;
mod pinned_candidate;
mod policy_authority;
mod unavailable_installation_authority;
mod unavailable_policy_authority;

use approved_installation::ApprovedInstallation;
use approved_policy::ApprovedPolicy;
use installation_authority::InstallationAuthority;
use installation_error::InstallationError;
use installation_key::InstallationKey;
#[cfg(test)]
use pin_candidate::pin_candidate;
use pinned_candidate::PinnedCandidate;
use policy_authority::PolicyAuthority;
#[cfg(test)]
use unavailable_installation_authority::UnavailableInstallationAuthority;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
