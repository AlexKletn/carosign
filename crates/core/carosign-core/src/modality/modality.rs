use super::UnknownModality;
use core::fmt;

/// Measurable.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modality {
    Fingerprint,
    Face,
    Iris,
    Voice,
}

impl fmt::Debug for Modality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fingerprint => "Fingerprint",
            Self::Face => "Face",
            Self::Iris => "Iris",
            Self::Voice => "Voice",
        })
    }
}

impl Modality {
    /// Value of wire enumeration `carosign.common.v1.Modality`.
    pub fn from_wire(value: i32) -> Result<Self, UnknownModality> {
        match value {
            1 => Ok(Self::Fingerprint),
            2 => Ok(Self::Face),
            3 => Ok(Self::Iris),
            4 => Ok(Self::Voice),
            0 => Err(UnknownModality(0)),
            other => Err(UnknownModality(u32::try_from(other).unwrap_or(0))),
        }
    }

    /// The value of the wire transfer.
    pub fn to_wire(self) -> i32 {
        match self {
            Self::Fingerprint => 1,
            Self::Face => 2,
            Self::Iris => 3,
            Self::Voice => 4,
        }
    }
}
