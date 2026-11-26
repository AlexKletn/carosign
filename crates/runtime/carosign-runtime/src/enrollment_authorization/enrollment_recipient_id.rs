use super::EnrollmentAuthorizationError;

/// Identifier of a specific trusted recipient within the platform.
/// The value itself does not prove that the recipient exists or is permitted.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnrollmentRecipientId([u8; 16]);

impl EnrollmentRecipientId {
    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, EnrollmentAuthorizationError> {
        if bytes == [0; 16] {
            return Err(EnrollmentAuthorizationError::Invalid);
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl core::fmt::Debug for EnrollmentRecipientId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("EnrollmentRecipientId(..)")
    }
}
