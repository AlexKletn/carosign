/// The reason for the failure of the trusted chain.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnrollmentAuthorizationError {
    Unavailable,
    Invalid,
    Denied,
    Expired,
    Revoked,
}
