use super::ClaimOutcome;
use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Owner {
    Platform,
    SystemFprintd,
    None,
}

///
/// A refused claim maps to `NotOwned`.
pub fn claim(current: Owner, requester: Owner) -> ClaimOutcome {
    match current {
        Owner::None => ClaimOutcome::Granted,
        Owner::Platform if requester == Owner::Platform => ClaimOutcome::Granted,
        other => ClaimOutcome::Refused { by: other },
    }
}

pub fn release(current: Owner, by: Owner) -> Owner {
    if current == by { Owner::None } else { current }
}

impl fmt::Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Platform => "платформа",
            Self::SystemFprintd => "fprintd",
            Self::None => "никто",
        })
    }
}

#[path = "owner.tests.rs"]
#[cfg(test)]
mod tests;
