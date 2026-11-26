/// How does the provider obtain the right to register (Document 31).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnrollmentAuthorization {
    /// No separate step: enough management permission and change log.
    None,
    /// You need a signed call: a link to authorization and digest.
    ///
    /// ⚠️ The token itself on the provider's wire ** does not go ** - only a link.
    Reference {
        /// Reference to authorization (16 bytes).
        reference: [u8; 16],
        /// SHA-256 call.
        challenge_digest: [u8; 32],
    },
}
