/// Element `[[profiles]]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileEntry {
    /// Profile name.
    pub id: String,
    /// The path to the profile file is relative.
    pub path: String,
}
