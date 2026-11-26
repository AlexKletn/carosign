/// Why is there no room for the operation?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionFailure {
    /// The service or owner limit is reached.
    Limit,
    /// Identifier's already taken.
    Duplicate,
}
