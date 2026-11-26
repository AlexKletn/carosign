//! Device Inventory: What the platform sees without running the provider.
//!
//! ⚠️ Rule 09: Profile mapping is done with inventory, not with the provider's output, so
//! it's just what the system says: subsystem, identifiers, and path.
//!
//! ⚠️ Address`/dev/…`) — **temporary**: it is never stored in the database.
//! The address changes when reconnecting, and registrations remain.

mod device_entry;
mod inventory_scan;
mod scan_error;

pub use device_entry::DeviceEntry;
pub use inventory_scan::InventoryScan;
pub use scan_error::ScanError;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
