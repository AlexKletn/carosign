//! Serialization of a one-time admission and recall from a trusted recipient.
//!
//! It is a local mechanism, not a confirmation of the security of another process.
//! The call of the recipient and all its feedback must pass through one copy; if the
//! recipient performs the action later or in another process, this blocking does not ensure
//! order there.

mod admission_error;
mod consumption_outcome;
mod serialized_enrollment_admission;
mod stage;

pub use admission_error::AdmissionError;
pub use consumption_outcome::ConsumptionOutcome;
pub use serialized_enrollment_admission::SerializedEnrollmentAdmission;
use stage::Stage;
