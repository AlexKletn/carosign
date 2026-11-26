#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InstallationError {
    Unavailable,
    MismatchedRecord,
    InvalidPath,
    UnsafeObject,
    Replaced,
    Kernel(i32),
}
