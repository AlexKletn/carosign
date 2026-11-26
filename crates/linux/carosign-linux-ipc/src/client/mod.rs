//! Customer side: reception, session, distribution, observation.
//!
//! ⚠️ The client side is the first frontier of trust. `CallerContext` (of creeds) and nothing
//! else: Authorities check the source of authority at each border.

pub mod accept;
pub mod dispatch;
pub mod mutation_events;
pub mod observe;
pub mod session;
pub mod transport;
pub mod verification_events;
