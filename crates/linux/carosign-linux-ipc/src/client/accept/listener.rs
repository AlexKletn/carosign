use super::ListenerError;

pub use crate::credentials::{CredentialError, PeerCredentials};

/// Listener.
///
/// ⚠️ No network address: `AF_UNIX` the platform is local, and `TCP` Not provided for in any
/// issue v1 (document 07).
#[derive(Debug)]
pub struct Listener {
    path: String,
    published: bool,
}

impl Listener {
    /// Prepared by a listener who has not yet published.
    pub fn new(path: &str) -> Result<Self, ListenerError> {
        if path.is_empty() || !path.starts_with('/') {
            return Err(ListenerError::BadPath);
        }
        // ⚠️ TCP is not supported at all: it’s not “unconfigured,” it’s a rejection.
        if path.starts_with("tcp://") || path.starts_with("http") {
            return Err(ListenerError::RemoteNotSupported);
        }
        Ok(Self {
            path: path.to_string(),
            published: false,
        })
    }

    /// Posts after recovery.
    pub fn publish(&mut self) -> Result<(), ListenerError> {
        if !self.published {
            self.published = true;
            return Ok(());
        }
        Err(ListenerError::AlreadyPublished)
    }

    pub fn is_published(&self) -> bool {
        self.published
    }

    /// The way.
    pub fn path(&self) -> &str {
        &self.path
    }
}
