//! Verification of the connection of events with the accepted operation; trust in the service
//! does not establish.

mod invalid_event;
mod observation;
mod observation_loop;
mod sequence_tracker;

pub use invalid_event::InvalidEvent;
pub use observation::Observation;
pub use observation_loop::VERIFICATION_WAIT_LIMIT;
pub use observation_loop::observe_verification_with_acceptance;
pub use observation_loop::{
    observe_verification_with_events, observe_verification_with_progress,
    observe_verification_with_result,
};
pub use sequence_tracker::VerificationEvents;
