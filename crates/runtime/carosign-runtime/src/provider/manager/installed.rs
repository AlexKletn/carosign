use crate::provider::packages::{Manifest, Profile};

/// Installed package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    /// Manifesto.
    pub manifest: Manifest,
    /// Profiles disassembled from files.
    pub profiles: Vec<Profile>,
    /// The package is entrusted to the directory owner, not declared itself.
    pub trusted: bool,
}
