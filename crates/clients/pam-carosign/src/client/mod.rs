//! The platform client is from PAM.
//!
//! ⚠️ The adapter is a client only (document 07): it doesn't open the sensor, it doesn't talk
//! to the ISP. All it does is one request and one answer.
//!
//! ⚠️ `PAM_USER` It goes into service as a clue; who the subject really is decides the source
//! of the authority.

mod adapter_error;
mod limits;
mod observation;
mod request;
mod selection;

pub use adapter_error::AdapterError;
pub use carosign_linux_ipc::client::verification_events::{
    Observation, observe_verification_with_acceptance,
};
pub use limits::FIELD_MAX;
pub use observation::observe_on_new_connection;
pub use observation::observe_system_verification;
pub use observation::observe_system_verification_for_modality;
pub use observation::observe_system_verification_with_acceptance;
pub use observation::observe_system_verification_with_control;
pub use observation::observe_verification;
pub use request::Request;
pub use request::verification_request;
pub use selection::select_authenticator;
use selection::select_for_account;
