use super::*;
use carosign_engine::{EffectSpec, TransitionIndex};

#[test]
fn timeout_is_retained_until_taken_and_duplicate_is_refused() {
    let owner = CleanupOwner::default();
    let registry = OperationRegistry::new();
    let id = OperationId::from_bytes([1; 16]).unwrap();
    let effect = PlannedEffect::at(EffectSpec::ReleaseResources, TransitionIndex::new(1), 0);
    futures_executor::block_on(owner.execute(
        &registry,
        id,
        effect,
        &crate::resources::NoBroker,
        std::future::ready(()),
    ))
    .unwrap();
    assert_eq!(
        futures_executor::block_on(owner.execute(
            &registry,
            id,
            effect,
            &crate::resources::NoBroker,
            std::future::ready(())
        )),
        Err(CleanupOwnerError::Duplicate)
    );
    assert!(matches!(owner.take(id), Some(CleanupResult::Interrupted)));
    assert!(owner.take(id).is_none());
}
