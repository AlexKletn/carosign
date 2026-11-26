//! The model of physical quiescence check of hardware issuance.
//!
//! The model doesn't check the hardware or remove the fence, and the source of the observations
//! is chosen by a trusted platform staff, not an worker of the provider.

mod hardware_quiescence;
mod observation;
mod stage;

pub use hardware_quiescence::HardwareQuiescence;
pub use observation::Observation;
pub use stage::Stage;
