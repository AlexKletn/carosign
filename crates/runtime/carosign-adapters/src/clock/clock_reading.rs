use carosign_core::ids::BootId;

/// Where does time come from?
pub trait ClockReading: Send + Sync {
    /// Monoton boot time, ns.
    fn boottime_ns(&self) -> u64;
    /// Load number.
    fn boot_id(&self) -> BootId;
}
