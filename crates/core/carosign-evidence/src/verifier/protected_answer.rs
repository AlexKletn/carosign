/// What the secure module said.
///
/// ⚠️ `Unknown` The legitimate answer is: then nothing is confirmed, and that is no reason to
/// assume the best.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProtectedAnswer {
    /// It's on hardware.
    Verified,
    Rejected,
    /// The token is not independently verified.
    Unverifiable,
    /// Module unavailable.
    Unavailable,
}
