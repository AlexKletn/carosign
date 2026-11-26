//! Receiving connections: Creds are removed ** when connected **.
//!
//! ⚠️ The connection is accepted with already removed credentials: changing the uid does not
//! change it later, and the right is checked separately.
//!
//! ⚠️ The listener is not published before the recovery is completed (document 05, step 7).

mod listener;
mod listener_error;

pub use listener::CredentialError;
pub use listener::Listener;
pub use listener::PeerCredentials;
pub use listener_error::ListenerError;
