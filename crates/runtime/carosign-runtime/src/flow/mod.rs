//! Transaction flows: verification, registration, deletion, cancellation, termination.
//!
//! This module sequences operations and reports the failure point. The engine decides
//! transitions, the executor handles effects, and ports supply authority.

pub mod cancel;
pub mod cycle;
pub mod enroll;
pub mod finalize;
pub mod remove;
pub mod verify;
