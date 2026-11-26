//! Device and authenticator: eras, address, links.
//!
//! ⚠️ `DeviceLocator` It's never on the database (document 03, invariant 1): the address
//! changes when you reconnect, but the registrations remain.
//!
//! `DeviceEpoch` It starts with **1, increases only by proven addition; zero is invalid;
//! overflow is a firm mistake; an era is what cuts off the messages of obsolete workers
//! (fencing).

mod authenticator;
mod authenticator_ref;
mod device_epoch;
mod device_locator;
mod invalid_epoch;
mod invalid_locator;

pub use authenticator::Authenticator;
pub use authenticator_ref::AuthenticatorRef;
pub use device_epoch::{DeviceEpoch, MAX_COUNTER};
pub use device_locator::{DEVICE_LOCATOR_MAX, DeviceLocator};
pub use invalid_epoch::InvalidEpoch;
pub use invalid_locator::InvalidLocator;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
