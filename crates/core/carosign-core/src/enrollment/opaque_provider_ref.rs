use super::InvalidOpaqueRef;
use core::fmt;

/// The maximum length of the opaque link of the provider, in bytes.
///
/// Limit from Table 13 and document 03 (invariant 6); wire field `provider_opaque_ref`
/// declared to 4,096 bytes.
pub const OPAQUE_REFERENCE_MAX: usize = 4096;

/// Non-transparent link of the provider to its registration.
///
/// The reference is opaque and conveys no template. The provider owns its format. The
/// platform checks only its length.
#[derive(Clone, PartialEq, Eq)]
pub struct OpaqueProviderRef(Vec<u8>);

impl OpaqueProviderRef {
    /// Accepts the link by only checking the length.
    ///
    /// Empty link is allowed: the platform provider may not have one.
    pub fn new(bytes: Vec<u8>) -> Result<Self, InvalidOpaqueRef> {
        if bytes.len() > OPAQUE_REFERENCE_MAX {
            return Err(InvalidOpaqueRef);
        }
        Ok(Self(bytes))
    }

    /// The bytes are fine.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Length in bytes.
    pub fn len_bytes(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for OpaqueProviderRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpaqueProviderRef({} байт)", self.0.len())
    }
}
