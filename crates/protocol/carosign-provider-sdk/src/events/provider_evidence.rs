/// Unverified material transmitted by an worker through a closed channel.
#[derive(Clone, PartialEq)]
pub enum ProviderEvidence {
    /// Provider link, not internal platform identifier
    Reference(Vec<u8>),
    /// An opaque token for an independent auditor.
    OpaqueToken(Vec<u8>),
}

impl zeroize::Zeroize for ProviderEvidence {
    fn zeroize(&mut self) {
        if let Self::OpaqueToken(token) = self {
            zeroize::Zeroize::zeroize(token);
        }
    }
}

impl Drop for ProviderEvidence {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(self);
    }
}
