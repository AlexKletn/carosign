use super::UnsupportedCapability;
use core::fmt;

/// How the user interacts with the sensor.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractionMode {
    Touch,
    /// Conduction.
    Swipe,
    /// The bottom of the screen.
    UnderDisplayOptical,
    LookAtCamera,
    Speak,
}

impl fmt::Debug for InteractionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Touch => "Touch",
            Self::Swipe => "Swipe",
            Self::UnderDisplayOptical => "UnderDisplayOptical",
            Self::LookAtCamera => "LookAtCamera",
            Self::Speak => "Speak",
        })
    }
}

impl InteractionMode {
    pub fn from_wire(value: i32) -> Result<Self, UnsupportedCapability> {
        match value {
            1 => Ok(Self::Touch),
            2 => Ok(Self::Swipe),
            3 => Ok(Self::UnderDisplayOptical),
            4 => Ok(Self::LookAtCamera),
            5 => Ok(Self::Speak),
            other => Err(UnsupportedCapability::Value(other)),
        }
    }
}
