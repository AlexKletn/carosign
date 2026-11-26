/// Candidate: Package plus profile that came up in the inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The name of the package.
    pub package: String,
    /// Profile name.
    pub profile: String,
}
