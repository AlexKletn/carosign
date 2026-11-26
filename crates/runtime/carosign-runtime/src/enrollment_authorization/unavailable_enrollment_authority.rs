use super::EnrollmentAuthorizationAuthority;

/// The default source does not allow the operation.
pub struct UnavailableEnrollmentAuthority;
impl EnrollmentAuthorizationAuthority for UnavailableEnrollmentAuthority {}
