//! Client session: greetings, queue of requests, events.
//!
//! ⚠️ Rule 07: Connection ** does not grant rights ** Session knows who came (creds removed
//! when connected), but the decision on each request is made by the source of authority.
//!
//! ⚠️ Waiting for a welcome is limited (5c): a silent client takes up space and cannot be
//! held forever.

mod client_session;
mod session_error;
mod session_phase;

pub use client_session::ClientSession;
pub use client_session::HELLO_TIMEOUT_MS;
pub use client_session::OPS_PER_CONNECTION;
pub use client_session::check;
pub use client_session::hello_frame;
pub use session_error::SessionError;
pub use session_phase::SessionPhase;
