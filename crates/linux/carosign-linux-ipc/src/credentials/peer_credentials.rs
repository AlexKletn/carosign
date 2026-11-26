use super::CredentialError;

use core::fmt;

/// Who's here?
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PeerCredentials {
    pid: u32,
    uid: u32,
    gid: u32,
}

impl PeerCredentials {
    /// A picture taken by the kernel when connected.
    ///
    /// ⚠️ Meanings come from `SO_PEERCRED`; they're only stored here.
    pub fn from_kernel(pid: u32, uid: u32, gid: u32) -> Result<Self, CredentialError> {
        if pid == 0 {
            return Err(CredentialError::Mismatch);
        }
        Ok(Self { pid, uid, gid })
    }

    /// PID defiant.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// UID defiant.
    pub fn uid(&self) -> u32 {
        self.uid
    }

    /// GID caller.
    pub fn gid(&self) -> u32 {
        self.gid
    }

    /// Coarse mark for `CallerContext`.
    ///
    /// ⚠️ The content is not a name or a path, it's just numbers, and it's just to
    /// distinguish one compound from another. Rights are not derived from that.
    pub fn binding(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[..4].copy_from_slice(&self.pid.to_be_bytes());
        out[4..8].copy_from_slice(&self.uid.to_be_bytes());
        out[8..12].copy_from_slice(&self.gid.to_be_bytes());
        out[31] = 1;
        out
    }
}

impl fmt::Debug for PeerCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The identity of the caller in the magazine does not go away entirely.
        f.write_str("PeerCredentials(..)")
    }
}

#[path = "peer_credentials.tests.rs"]
#[cfg(test)]
mod tests;
