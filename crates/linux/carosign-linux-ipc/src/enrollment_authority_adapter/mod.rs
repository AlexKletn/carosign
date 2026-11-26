//! Adapter of a trusted registration source to individual IPC channels.
//!
//! The adapter verifies correlation, order, and recall; only the verified composition
//! confirms the trusted origin of the process, secure connection, and authority behind it.
//! The adapter itself does not confirm the safety of the sink and is not a reason to include
//! registration.

mod ipc_enrollment_authority;
mod phase;
mod session;
mod trusted_enrollment_connector;

pub use ipc_enrollment_authority::IpcEnrollmentAuthority;
use phase::Phase;
use session::Session;
pub use trusted_enrollment_connector::TrustedEnrollmentConnector;
