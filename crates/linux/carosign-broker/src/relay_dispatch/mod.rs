//! Closed list of shipments and transitions of one shipment of an broker.
//!
//! It's not transportation, it's not a source of trust. `begin` Check the policy, scope,
//! deadline, recall, and reinstatement and serialize those checks when recording begins.

mod direction;
mod invalid_transition;
mod pending_packet;
mod send_guard;
mod send_state;

pub use direction::Direction;
pub use invalid_transition::InvalidTransition;
pub(crate) use pending_packet::PendingPacket;
pub use send_guard::SendGuard;
pub use send_guard::direction;
pub use send_state::SendState;
