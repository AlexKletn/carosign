use super::*;

#[test]
fn published_identity_keeps_epoch_until_loss_or_peer_change() {
    let metadata = std::fs::metadata(".").unwrap();
    let peer = EndpointIdentity::of(&metadata, 10);
    let successor = EndpointIdentity::of(&metadata, 11);
    let descriptor = AuthenticatorDescriptor {
        authenticator_id: vec![3; 16],
        ..Default::default()
    };
    let id = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let mut epochs = InventoryEpochs::default();
    let mut update = |identity: Option<EndpointIdentity>| {
        epochs.update(
            identity
                .into_iter()
                .map(|id| (descriptor.clone(), id))
                .collect(),
            [id],
        )
    };
    let first = update(Some(peer)).unwrap()[0].device_epoch;
    assert_eq!(update(Some(peer)).unwrap()[0].device_epoch, first);
    let second = update(Some(successor)).unwrap()[0].device_epoch;
    assert!(second > first);
    let unavailable = update(None).unwrap();
    assert_eq!(unavailable[0].device_epoch, second);
    assert!(!unavailable[0].available);
    assert!(update(Some(successor)).unwrap()[0].device_epoch > second);
    assert!(epochs.update(Vec::new(), []).unwrap().is_empty());
    assert!(epochs.binding(id).is_none());
}
