use core::fmt;

/// Where did the test come from?
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provenance {
    /// The provider claims a match without a token (Document 29, D-06).
    ///
    /// ⚠️ This is a *statement*, not a hardware proof: the platform winds it up itself, from
    /// trusted IPC and correlation, and it’s only good for policies that don’t require hardware
    /// protection.
    ProviderAssertion,
    /// Checked out the program player.
    SoftwareMatcher,
    /// Checked the protected module (hardware path).
    ProtectedModule,
}

impl fmt::Debug for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ProviderAssertion => "ProviderAssertion",
            Self::SoftwareMatcher => "SoftwareMatcher",
            Self::ProtectedModule => "ProtectedModule",
        })
    }
}
