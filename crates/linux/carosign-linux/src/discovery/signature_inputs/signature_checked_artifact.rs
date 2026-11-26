use std::os::fd::AsFd;
use std::os::fd::BorrowedFd;
use std::os::fd::OwnedFd;

/// A snapshot verified by the gpgv assistant does not confirm the origin of the gpgv, the
/// keyset or the right of installation.
#[derive(Debug)]
pub(in crate::discovery) struct SignatureCheckedArtifact {
    pub(super) artifact: OwnedFd,
    pub(super) signature: OwnedFd,
    pub(super) primary_fingerprint: String,
    pub(super) primary_key_version: u8,
    pub(super) keyring_verity: Option<[u8; 32]>,
    pub(super) verifier_verity: Option<[u8; 32]>,
}

impl SignatureCheckedArtifact {
    /// A version of the single main key in the verified keyring shot.
    pub(in crate::discovery) fn primary_key_version(&self) -> u8 {
        self.primary_key_version
    }
    /// The extract should read the verified image, not the original path.
    pub(in crate::discovery) fn as_fd(&self) -> BorrowedFd<'_> {
        self.artifact.as_fd()
    }

    /// A sealed snapshot of the signature refers to the same verification call.
    pub(in crate::discovery) fn signature_fd(&self) -> BorrowedFd<'_> {
        self.signature.as_fd()
    }

    /// Keeps the connection of the check with the inputs for subsequent verification by the
    /// owner.
    /// This does not confirm the authority of the key or the origin of gpgv.
    pub(in crate::discovery) fn signature_inputs(&self) -> (&str, Option<&[u8; 32]>) {
        (&self.primary_fingerprint, self.keyring_verity.as_ref())
    }

    /// Measurement of the verifier file does not confirm its dynamic environment.
    pub(in crate::discovery) fn verifier_verity(&self) -> Option<&[u8; 32]> {
        self.verifier_verity.as_ref()
    }
}
