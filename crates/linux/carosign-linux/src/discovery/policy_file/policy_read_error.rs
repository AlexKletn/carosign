#[derive(Debug, PartialEq, Eq)]
pub(in crate::discovery) enum PolicyReadError {
    InvalidPath,
    InvalidLimit,
    UnsafeObject,
    TooLarge,
    Changed,
    Kernel(i32),
}
