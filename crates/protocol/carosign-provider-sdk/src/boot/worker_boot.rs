use super::BootError;

/// Check your surroundings before you talk.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Boot {
    /// The channel from the platform has already been issued.
    pub channel: bool,
    /// The hardware isn't open yet.
    pub device_closed: bool,
}

impl Boot {
    /// Can we start a conversation?
    pub fn check(self) -> Result<(), BootError> {
        if !self.channel {
            return Err(BootError::NoChannel);
        }
        if !self.device_closed {
            return Err(BootError::DeviceOpenTooEarly);
        }
        Ok(())
    }
}

#[path = "worker_boot.tests.rs"]
#[cfg(test)]
mod tests;
