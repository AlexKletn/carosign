use super::*;

#[test]
fn legacy_under_display_mode_exposes_capture_method_and_placement() {
    let mut descriptor = CapabilityDescriptor {
        interaction_modes: vec![InteractionMode::InteractionUnderDisplayOptical as i32],
        ..Default::default()
    };
    normalize_capture_metadata(&mut descriptor);
    normalize_capture_metadata(&mut descriptor);
    assert_eq!(descriptor.placement, SensorPlacement::UnderDisplay as i32);
    assert_eq!(descriptor.capture_methods, [CaptureMethod::Optical as i32]);
    assert_eq!(
        descriptor.interaction_modes,
        [InteractionMode::InteractionUnderDisplayOptical as i32],
    );
}
