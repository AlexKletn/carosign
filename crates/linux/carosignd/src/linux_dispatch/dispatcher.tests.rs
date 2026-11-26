use super::*;
use carosign_protocol::carosign::common::v1::{CapabilityDescriptor, Modality, ProviderOperation};

#[test]
fn declaration_is_not_readiness_and_empty_or_unknown_operations_are_unavailable() {
    assert!(!available_capabilities(None, true));
    let mut capabilities = CapabilityDescriptor {
        modality: Modality::Fingerprint as i32,
        ..Default::default()
    };
    assert!(!available_capabilities(Some(&capabilities), true));
    capabilities.operations = vec![0, i32::MAX];
    assert!(!available_capabilities(Some(&capabilities), true));
    capabilities
        .operations
        .push(ProviderOperation::ProviderVerify as i32);
    assert!(!available_capabilities(Some(&capabilities), false));
    assert!(available_capabilities(Some(&capabilities), true));
    for modality in [Modality::Face, Modality::Iris, Modality::Voice] {
        capabilities.modality = modality as i32;
        assert!(!available_capabilities(Some(&capabilities), true));
    }
}
