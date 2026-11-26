//! Validates untrusted wire messages after decoding.
//!
//! This layer checks identifier values, enum discriminants, field lengths, and payload
//! shape. Parsing can drop unknown fields and cannot establish authority. See specification 20.

mod broker;
mod capture_feedback;
mod capture_metadata;
mod client;
mod declared_assurance;
mod field;
mod provider;
mod wire_error;

pub use broker::check_broker_frame;
pub use capture_feedback::{
    capture_feedback_is_declared, capture_state_is_declared, check_capture_feedback,
    valid_capture_state,
};
pub use capture_metadata::normalize_capture_metadata;
pub use client::{check_client_frame, check_subject_selector};
pub use declared_assurance::check_declared_assurance;
pub use field::{
    bounded, bounded_str, check_frame_sequence, check_frame_size, check_version, known_enum,
    optional_id, require_id, specified,
};
pub use provider::{
    check_bind, check_provider_envelope, check_verification_outcome, check_worker_hello,
};
pub use wire_error::WireError;
