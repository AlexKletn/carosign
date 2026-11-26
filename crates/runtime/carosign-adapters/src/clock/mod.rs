//! Clock: real and step.
//!
//! ⚠️ Deadline created from the system clock does not experience a reboot (Document 05).
//! `TimeSource` It is obliged to give the download number along with time - without this, the
//! "term" through the restart will become a stranger.
//!
//! ⚠️ Real. `clock_gettime(CLOCK_BOOTTIME)` dwells `carosign-linux`There are two
//! incarnations that can be verified: stepping clocks and “system” with a transmitted source.

mod clock_reading;
mod step_clock;
mod system_clock;

pub use clock_reading::ClockReading;
pub use step_clock::StepClock;
pub use system_clock::SystemClock;
