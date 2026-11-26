//! Device Inventory: What the platform sees without running the provider.
//!
//! ⚠️ Address of the device (`DeviceLocator`) **temporary. It is never in the database: the
//! address changes when reconnected, and registrations remain (document 03, invariant 1).
//!
//! ⚠️ Inventory fills up `carosign-linux`This is just what you need to compare profiles.

mod inventory;
mod inventory_entry;

pub use inventory::Inventory;
pub use inventory_entry::InventoryEntry;
