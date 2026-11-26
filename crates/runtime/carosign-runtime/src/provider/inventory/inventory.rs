use super::InventoryEntry;

/// A snapshot of inventory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    entries: Vec<InventoryEntry>,
}

impl Inventory {
    /// Empty inventory is a legal condition: there may be no providers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the device.
    pub fn push(&mut self, entry: InventoryEntry) {
        self.entries.push(entry);
    }

    /// I can see that.
    pub fn entries(&self) -> &[InventoryEntry] {
        &self.entries
    }

    /// How many devices.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// It's empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
