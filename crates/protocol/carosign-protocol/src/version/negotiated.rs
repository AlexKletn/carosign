use super::VersionMismatch;

use crate::limits;

/// Agreed minor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Negotiated {
    /// Senior (always 1).
    pub major: u32,
    pub minor: u32,
}

/// Confirms the version with the remote side.
pub fn negotiate(
    their_major: u32,
    their_min: u32,
    their_max: u32,
) -> Result<Negotiated, VersionMismatch> {
    if their_major != limits::MAJOR || their_min > their_max {
        return Err(VersionMismatch {
            our_major: limits::MAJOR,
            their_major,
        });
    }

    let lower = their_min.max(limits::MINOR_BASELINE);
    let upper = their_max.min(limits::MINOR_MAX);
    if lower <= upper {
        Ok(Negotiated {
            major: limits::MAJOR,
            minor: upper,
        })
    } else {
        Err(VersionMismatch {
            our_major: limits::MAJOR,
            their_major,
        })
    }
}

#[path = "negotiated.tests.rs"]
#[cfg(test)]
mod tests;
