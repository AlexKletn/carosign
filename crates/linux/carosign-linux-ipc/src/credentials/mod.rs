//! Peer-to-peer creeds: who came to us?
//!
//! ⚠️ The main thing is to keep: `SO_PEERCRED` It's not a right, it's not a root, it's a
//! right, it's a right that checks the source of authority at each border (document 08).
//!
//! ⚠️ The picture is taken when connected: changing the uid process later does not change it,
//! and that is exactly what we need.

mod credential_error;
mod peer_credentials;

pub use credential_error::CredentialError;
pub use peer_credentials::PeerCredentials;
