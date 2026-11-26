//! Platform commands disassembled into typed views.
//!
//! This layer validates identifiers as nonzero 16-byte values, checks enum values, and
//! checks the phase before parsing. Invalid commands never reach hardware.

mod command_error;
mod decoded_command;
mod enrollment_authorization;
mod enrollment_change;

pub use command_error::CommandError;
pub use decoded_command::Command;
pub use decoded_command::parse;
pub use enrollment_authorization::EnrollmentAuthorization;
pub use enrollment_change::EnrollmentChange;
