use super::ProtectedPolicySnapshot;
use std::io;
use std::path::Path;

/// Fixed sources of the proposed system composition.
///
/// Name selection does not prove trust: a challenge `open` The only thing that checks is the
/// path and the image, and the contents and the sole owner remain the responsibility of the
/// composition, and production is not included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemTrustSource {
    /// The initial systemic root of administrative policy.
    RootPolicy,
    /// Installation verification key ring.
    InstallerKeyring,
}

impl SystemTrustSource {
    /// The name is not taken from the client or environment variables.
    pub fn path(self) -> &'static Path {
        Path::new(match self {
            Self::RootPolicy => "/etc/carosign/trust/root.conf",
            Self::InstallerKeyring => "/etc/carosign/trust/installers.gpg",
        })
    }

    /// Secures the file by system name with path check.
    pub fn open(self) -> io::Result<ProtectedPolicySnapshot> {
        ProtectedPolicySnapshot::open(self.path())
    }
}
