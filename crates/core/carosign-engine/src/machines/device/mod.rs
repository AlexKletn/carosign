//! Machine Device: available, busy, active, calming down, unclear.
//!
//! Table - document 09, section "Device States".
//!
//! ⚠️ Important:`DeviceState::Uncertain`] **not ** becomes [`DeviceState::Available`] with
//! one assurance. Only a confirmed recovery can bring the device back into operation.

mod device_event;
mod device_state;

pub use device_event::DeviceEvent;
pub use device_state::{DeviceState, can_reserve, next_state};
