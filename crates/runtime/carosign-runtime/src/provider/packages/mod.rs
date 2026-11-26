//! Provider package manifest and device profile.
//!
//! ⚠️ Manifesto -- **intrusted data. **Only the schema fields are read, all the extra is
//! failure. `deny_unknown_fields` An unfamiliar field in the manifesto can mean “this version
//! does more,” and to swallow it silently is to make a decision that has not been verified.
//!
//! ⚠️ The manifesto does not give away anything: no rights, no device, no trust. It only
//! announces.

pub mod manifest;
pub mod profile;
pub mod resolve;

pub use manifest::{
    InstanceModel, Manifest, ManifestError, PackageSection, ProfileEntry, ProtocolSection,
    WorkerSection,
};
pub use profile::{MatchRule, Profile, ProfileError};
pub use resolve::{Candidate, Resolution, resolve};

/// Relative path test.
pub use manifest::check_relative_path;
/// Discussion of the manifesto (short pseudonym).
pub use manifest::parse as parse_manifest;
/// Profile analysis.
pub use profile::parse as parse_profile;
