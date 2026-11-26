//! Exchange on a separate private channel of the resource owner.
//!
//! The code does not run the owner and does not give authority: the trusted supervisor is
//! obliged to verify the origin of the connected data. `SOCK_SEQPACKET`Hold the durable fence
//! until called and independently confirm the identity of the received FDs.

mod owner_session;
mod reply;

pub use owner_session::OwnerSession;
pub use owner_session::prepare_once;
pub use reply::Reply;
