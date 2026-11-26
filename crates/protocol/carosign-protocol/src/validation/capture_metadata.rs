use crate::carosign::common::v1::{
    CapabilityDescriptor, CaptureMethod, InteractionMode, SensorPlacement,
};

pub fn normalize_capture_metadata(descriptor: &mut CapabilityDescriptor) {
    if !descriptor
        .interaction_modes
        .contains(&(InteractionMode::InteractionUnderDisplayOptical as i32))
    {
        return;
    }
    if descriptor.placement == SensorPlacement::Unspecified as i32 {
        descriptor.placement = SensorPlacement::UnderDisplay as i32;
    }
    let optical = CaptureMethod::Optical as i32;
    if !descriptor.capture_methods.contains(&optical) {
        descriptor.capture_methods.push(optical);
    }
}

#[path = "capture_metadata.tests.rs"]
#[cfg(test)]
mod tests;
