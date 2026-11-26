/// Section `[package]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageSection {
    /// Reverse domain.
    pub id: String,
    /// The "number. number. number" version.
    pub version: String,
}
