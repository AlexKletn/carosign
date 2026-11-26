use carosign_core::identity::SubjectRef;
use carosign_core::ids::AuthenticatorId;

/// Scope of change: `authority ‖ subject ‖ authenticator`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Scope {
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorId,
}

impl Scope {
    /// Canonical entry: 48 bytes.
    pub fn canonical(&self) -> [u8; 48] {
        let mut out = [0u8; 48];
        out[..32].copy_from_slice(&self.subject.to_canonical());
        out[32..].copy_from_slice(self.authenticator.as_bytes());
        out
    }
}
