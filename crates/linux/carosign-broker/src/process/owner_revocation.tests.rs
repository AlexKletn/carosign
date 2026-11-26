use super::*;

#[test]
fn revoke_requires_exact_scope_idle_relay_and_confirmed_owner() {
    use carosign_protocol::resource_owner::Quiescence;
    let held = Scope {
        lease_id: vec![1; 16],
        owner: None,
        claims: Vec::new(),
    };
    let other = Scope {
        lease_id: vec![2; 16],
        ..held.clone()
    };
    for (wanted, pending, events) in [
        (None, false, false),
        (Some(&other), false, false),
        (Some(&held), true, false),
        (Some(&held), false, true),
    ] {
        assert!(
            revoke_owner(Some(&held), wanted, pending, events, || panic!(
                "отзыв недопустим"
            ))
            .is_err()
        );
    }
    for observation in [
        Quiescence::Draining,
        Quiescence::LeaseRevoked,
        Quiescence::ExclusiveLeaseRevoked,
        Quiescence::ExclusiveQuiesced,
    ] {
        let outcome = revoke_owner(Some(&held), Some(&held), false, false, || Ok(observation));
        assert_eq!(outcome.is_ok(), observation != Quiescence::Draining);
    }
    assert!(revoke_owner(Some(&held), Some(&held), false, false, || Err(())).is_err());
}
