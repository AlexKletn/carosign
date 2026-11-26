use super::*;

/// The error after initialization forever expires this instance of the watch.
pub(crate) struct BootClock {
    boot: BootId,
    state: Mutex<ClockState>,
}

impl BootClock {
    /// Does not create a watch if the download number or the original time is not available.
    pub(crate) fn initialize() -> Result<Self, SysError> {
        let boot = BootId::from_bytes(sys::boot_id()?).map_err(|_| SysError {
            errno: libc::EINVAL,
        })?;
        let last = sys::boottime_ns()?;
        Ok(Self {
            boot,
            state: Mutex::new(ClockState {
                last,
                failed: false,
            }),
        })
    }
}

impl TimeSource for BootClock {
    fn now(&self) -> BootInstant {
        // Taking time under locking does not allow the results of different streams to be
        // rearranged. Failure to lock or core does not extend the deadlines.
        let nanos = match self.state.lock() {
            Ok(mut state) => state.observe(sys::boottime_ns()),
            Err(_) => u64::MAX,
        };
        BootInstant::new(self.boot, nanos)
    }

    fn boot_id(&self) -> BootId {
        self.boot
    }
}
