/// What's allowed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    /// Testing.
    Verify,
    /// Registry.
    Enroll,
    /// Removal.
    Remove,
    /// Surveillance.
    Observe,
    Inventory,
    Cancel,
}
