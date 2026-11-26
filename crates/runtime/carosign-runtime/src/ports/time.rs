use carosign_core::ids::BootId;
use carosign_core::operation::context::BootInstant;

/// Watch and load.
pub trait TimeSource: Send + Sync {
    /// Now, along with the download number.
    fn now(&self) -> BootInstant;
    /// Load number.
    fn boot_id(&self) -> BootId;
}
