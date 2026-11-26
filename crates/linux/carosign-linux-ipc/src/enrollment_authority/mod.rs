//! Private channel to trusted source of registration confirmation.
//!
//! It's a transport, not a source of authority, and it's a duty of the lineup to run both
//! parties with confidence, hand over closed descriptors, and connect the area to live
//! delivery, politics, and a specific recipient. `SO_PEERCRED` For socketpair, it reflects
//! the creator of the socket; it does not prove who saved the descriptor after launch.

mod channel;
mod message;
mod peer;
mod scope;
mod validation;

pub use message::{
    Approved, Authorization, Empty, GrantBinding, OwnerBinding, PlanBinding, PlanChange, Request,
    Response, Validate, request, response,
};
pub use peer::ExpectedPeer;

pub use channel::AuthorityChannel;
