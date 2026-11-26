//! Surveillance of the operation: where to continue and where to fail.
//!
//! ⚠️ `after_sequence = 0` If the beginning of the story is lost, the client receives a * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * *

/// Subscription response.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ObserveStart {
    /// There are no more valid numbers.
    Exhausted,
    /// What numbers do we start with?
    From {
        /// Number one.
        sequence: u64,
    },
    /// The beginning is lost: first the picture, then the events from this number.
    GapThenSnapshot {
        /// Which number we're gonna go from.
        resume_from: u64,
    },
}

/// Deciding where to start.
///
/// ⚠️ The limit of history is set `HISTORY_MAX` In runtime, there's only a solution.
pub fn start(after_sequence: u64, oldest_available: Option<u64>) -> ObserveStart {
    let Some(next) = after_sequence.checked_add(1) else {
        return ObserveStart::Exhausted;
    };
    match oldest_available {
        None => ObserveStart::From { sequence: next },
        Some(oldest) if next < oldest => ObserveStart::GapThenSnapshot {
            resume_from: oldest,
        },
        Some(_) => ObserveStart::From { sequence: next },
    }
}

#[path = "observe.tests.rs"]
#[cfg(test)]
mod tests;
