//! Translation of the system image into profile comparison inventory.
//!
//! The image alone does not confirm the origin of the packages, the stable identity of the
//! device, or the right to start the worker.

use carosign_linux::discovery::inventory::InventoryScan;
use carosign_runtime::provider::inventory::{Inventory, InventoryEntry};

/// The system source did not confirm the completeness of the surveyed area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncompleteInventory;

/// Translates the full picture, saving temporary addresses and system features.
///
/// The current system source only examines `hidraw`A successful translation does not mean
/// completeness for other subsystems, but the caller must check that the surveyed area covers
/// the rules of the profiles in question.
/// An incomplete picture never turns into a reliable empty inventory.
pub fn from_linux_scan(scan: &InventoryScan) -> Result<Inventory, IncompleteInventory> {
    convert(scan.is_incomplete(), scan.entries())
}

fn convert(
    incomplete: bool,
    entries: &[carosign_linux::discovery::inventory::DeviceEntry],
) -> Result<Inventory, IncompleteInventory> {
    if incomplete {
        return Err(IncompleteInventory);
    }
    let mut inventory = Inventory::new();
    for entry in entries {
        inventory.push(InventoryEntry {
            locator: entry.locator.clone(),
            signals: entry.signals.clone(),
        });
    }
    Ok(inventory)
}

#[path = "inventory.tests.rs"]
#[cfg(test)]
mod tests;
