use core::fmt;

/// What's wrong with the launch?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BootError {
    NoChannel,
    /// The hardware is open to the tether.
    DeviceOpenTooEarly,
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoChannel => "канал от платформы не выдан",
            Self::DeviceOpenTooEarly => "железо открыто до привязки",
        })
    }
}

impl core::error::Error for BootError {}
