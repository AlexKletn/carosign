use carosign_core::ids::EnrollmentId;

/// That a status check should compare.
///
/// ⚠️ For `Enroll(replace)` The initial state is perhaps the **old* registration, not the
/// absence: without it, “nothing new” would be falsely read as “nothing has changed.”
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExpectedStates {
    /// The initial expected state (before the operation).
    pub pre: EnrollmentId,
    /// Target expected state (after operation).
    pub desired: EnrollmentId,
}
