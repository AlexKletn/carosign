use crate::material::EvidenceMaterialRef;

use super::*;

fn reference() -> EvidenceMaterialRef {
    let slot = crate::store::EvidenceSlot::new(
        carosign_core::ids::EvidenceId::from_bytes([1; 16]).unwrap(),
        carosign_core::ids::OperationId::from_bytes([2; 16]).unwrap(),
        carosign_core::operation::context::BootDeadline::new(
            carosign_core::ids::BootId::from_bytes([3; 16]).unwrap(),
            20,
        ),
    );
    slot.hold(
        zeroize::Zeroizing::new(vec![1]),
        carosign_core::operation::context::BootInstant::new(
            carosign_core::ids::BootId::from_bytes([3; 16]).unwrap(),
            10,
        ),
    )
    .unwrap()
}

#[test]
fn material_is_single_use() {
    let reference = reference();
    let mut held = Held::hold(reference);
    assert!(held.take([2u8; 16]).is_ok());
    assert_eq!(held.take([2u8; 16]), Err(RoutingError::AlreadyConsumed));
}

#[test]
fn material_belongs_to_one_operation() {
    let reference = reference();
    let mut held = Held::hold(reference);
    assert_eq!(held.take([3u8; 16]), Err(RoutingError::WrongOperation));
    // Failure does not burn the material.
    assert_eq!(held.state(), MaterialState::Held);
}

#[test]
fn invalidation_is_scoped_and_prevents_reuse() {
    let reference = reference();
    let mut held = Held::hold(reference);
    assert_eq!(held.invalidate([3; 16]), Err(RoutingError::WrongOperation));
    assert_eq!(held.state(), MaterialState::Held);
    assert_eq!(held.invalidate([2; 16]), Ok(()));
    assert_eq!(held.state(), MaterialState::Destroyed);
    assert_eq!(held.take([2; 16]), Err(RoutingError::AlreadyConsumed));
}
