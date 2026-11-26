#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FprintdPhase {
    Idle,
    Verifying,
    Enrolling,
    /// The adapter has no claim to the device.
    NotOwned,
}
