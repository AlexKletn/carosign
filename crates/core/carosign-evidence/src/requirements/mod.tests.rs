use carosign_core::assurance::AssuranceClass;
use carosign_core::operation::context::VerificationPurpose;

use super::*;

#[test]
fn contradictory_key_requirements_never_accept_any_class() {
    let invalid = EvidenceRequirements {
        purpose: VerificationPurpose::KeyOperation,
        hardware_required: false,
        fresh_challenge: true,
    };
    assert!(invalid.validate().is_err());
    for class in [
        AssuranceClass::Unverified,
        AssuranceClass::SoftwareAsserted,
        AssuranceClass::HardwareVerified,
    ] {
        assert!(!invalid.accepts(class));
    }
}

#[test]
fn software_assertion_never_satisfies_key_operation() {
    let key = EvidenceRequirements::key_operation();
    assert!(!key.accepts(AssuranceClass::SoftwareAsserted));
    assert!(!key.accepts(AssuranceClass::Unverified));
    assert!(key.accepts(AssuranceClass::HardwareVerified));
    assert!(EvidenceRequirements::local_login().accepts(AssuranceClass::SoftwareAsserted));
}
