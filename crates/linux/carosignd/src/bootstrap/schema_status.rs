/// What is known about the storage scheme during recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaStatus {
    /// The marker is missing or unknown.
    Unverified,
    /// The marker corresponds to v3 with no incomplete migration.
    V3,
    /// The base of the first installation v4 and supported records are checked.
    V4,
}
