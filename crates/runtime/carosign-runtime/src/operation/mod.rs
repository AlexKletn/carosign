//! Operations: actor, mail, registry, owner, observers, events.
//!
//! ⚠️ One actor is one state writer. The rest of it is observation: owner, observers, the
//! flow of events.

pub mod actor;
pub mod bindings;
pub mod events;
pub mod hub;
pub mod mailbox;
pub mod observer;
pub mod owner;
pub mod registry;
