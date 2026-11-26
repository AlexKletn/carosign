use super::*;

#[test]
fn unknown_capability_values_are_errors() {
    assert!(TemplateCustody::from_wire(0).is_err());
    assert!(TemplateCustody::from_wire(9).is_err());
    assert!(InteractionMode::from_wire(6).is_err());
    assert_eq!(
        InteractionMode::from_wire(4),
        Ok(InteractionMode::LookAtCamera)
    );
    assert_eq!(InteractionMode::from_wire(5), Ok(InteractionMode::Speak));
    assert!(RecoveryMode::from_wire(-1).is_err());
    assert_eq!(RecoveryMode::from_wire(1), Ok(RecoveryMode::NotSupported));
}

#[test]
fn not_supported_recovery_means_no_reconcile_path() {
    assert!(!RecoveryMode::NotSupported.can_reconcile());
    assert!(RecoveryMode::SafeStateQuery.can_reconcile());
    assert!(RecoveryMode::ProviderManaged.can_reconcile());
}

#[test]
fn default_fingerprint_descriptor_covers_baseline() {
    let d = CapabilityDescriptor::fingerprint_touch();
    assert!(d.can_verify() && d.can_enroll() && d.can_remove());
    assert!(!d.recovery.can_reconcile());
}
