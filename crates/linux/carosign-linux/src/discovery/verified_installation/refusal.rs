#[derive(Debug, PartialEq, Eq)]
pub(in crate::discovery) enum Refusal {
    Unavailable,
    Mismatch,
    InvalidData,
    UnsafePath,
    UnsafeObject,
    Changed,
    Kernel(i32),
}
