use crate::ids::EvidenceId;
use crate::ids::OperationId;

use super::*;

#[test]
fn evidence_ref_does_not_replay_across_operations() {
    let evidence = EvidenceId::from_bytes([1u8; 16]).expect("валид");
    let first = OperationId::from_bytes([2u8; 16]).expect("валид");
    let second = OperationId::from_bytes([3u8; 16]).expect("валид");
    let reference = EvidenceRef::new(evidence, first);
    assert!(reference.belongs_to(first));
    assert!(!reference.belongs_to(second));
}
