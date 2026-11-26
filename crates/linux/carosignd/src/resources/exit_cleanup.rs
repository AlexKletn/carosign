/// Completion of processes proves the closure of their FDs, but not the rest of the device.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitCleanup {
    ProcessOnly,
    HardwareUnconfirmed,
}
