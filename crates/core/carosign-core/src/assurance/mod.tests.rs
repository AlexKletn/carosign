use crate::ids::AuthenticatorId;
use crate::ids::EnrollmentId;
use crate::ids::EvidenceId;
use crate::ids::OperationId;

use super::*;

fn parts() -> (EvidenceId, OperationId, EnrollmentId, AuthenticatorId) {
    (
        EvidenceId::from_bytes([1u8; 16]).expect("валид"),
        OperationId::from_bytes([2u8; 16]).expect("валид"),
        EnrollmentId::from_bytes([3u8; 16]).expect("валид"),
        AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
    )
}

#[test]
fn software_never_satisfies_key_operation() {
    let (e, op, en, au) = parts();
    let software = AssuranceAssessment::record(e, AssuranceClass::SoftwareAsserted, op, en, au);
    assert!(!software.satisfies_key_operation());
    assert!(
        !AssuranceAssessment::record(e, AssuranceClass::Unverified, op, en, au)
            .satisfies_key_operation()
    );
    assert!(
        AssuranceAssessment::record(e, AssuranceClass::HardwareVerified, op, en, au)
            .satisfies_key_operation()
    );
}
