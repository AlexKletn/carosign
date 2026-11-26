/// The reason why the image cannot be used to prepare the launch.
#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotRefusal {
    /// The marker does not confirm the normal v4 mode.
    UnsupportedSchema,
    /// The table or its data is inaccessible or damaged.
    Corrupt,
    /// There is no packet or installation record.
    NotFound,
    /// Installation withdrawn.
    Revoked,
    /// The recording changed after the files were prepared.
    Changed,
}
