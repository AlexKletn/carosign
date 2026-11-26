use core::fmt;

/// The ISP's claims are **untrusted**.
///
/// ⚠️ This is the raw data of the provider along with the mark “I claim so”.
/// There is no guarantee from them: they fall into the inspector, not into the assessment.
#[derive(Clone, PartialEq, Eq)]
pub struct EvidenceClaims {
    /// Who's claiming.
    pub authenticator: crate::ids::AuthenticatorId,
    /// Proof format, as the provider called it (data, not trust).
    pub claimed_format_id: Option<crate::provider::PackageName>,
}

impl fmt::Debug for EvidenceClaims {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvidenceClaims")
            .field("authenticator", &"..")
            .field("has_format", &self.claimed_format_id.is_some())
            .finish()
    }
}

impl EvidenceClaims {
    /// Metadata statements; fields remain untrusted data.
    pub fn new(
        authenticator: crate::ids::AuthenticatorId,
        claimed_format_id: Option<crate::provider::PackageName>,
    ) -> Self {
        Self {
            authenticator,
            claimed_format_id,
        }
    }
}
