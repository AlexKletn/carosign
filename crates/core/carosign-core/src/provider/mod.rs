//! Provider and its device profile.
//!
//! ⚠️ There are only **values **: who declared what and how it is compared.
//! There is no running of processes - this is `carosign-runtime` and `carosign-linux`.
//!
//! The names in this module are unchanged: `ProviderId` (the implementation family) and
//! `PackageId` (installed package) - **different things**, and both are not enough for
//! hardware identity. `package_name` It exists only for people and policy decisions.

mod invalid_package_name;
mod invalid_version;
mod package;
mod package_name;
mod package_version;
mod registered_profile;

pub use invalid_package_name::InvalidPackageName;
pub use invalid_version::InvalidVersion;
pub use package::Package;
pub use package_name::{PACKAGE_NAME_MAX, PackageName};
pub use package_version::PackageVersion;
pub use registered_profile::RegisteredProfile;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
