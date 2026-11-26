use super::*;
use carosign_protocol::carosign::{
    broker_relay::v1::{Claim, Owner},
    common::v1::{AuthenticatorRef, SubjectRef},
};

#[test]
fn permit_binds_lease_subject_authenticator_operation_and_mutation() {
    let context = Context {
        worker: [1; 16],
        epoch: 7,
        broker: [3; 16],
        boot: [4; 16],
    };
    let mut scope = Scope {
        lease_id: vec![6; 16],
        owner: Some(Owner {
            operation_id: vec![8; 16],
            worker_instance: vec![1; 16],
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        claims: vec![Claim {
            resource_id: vec![7; 16],
            mode: 1,
        }],
    };
    let subject = SubjectRef {
        authority_id: vec![10; 16],
        subject_id: vec![11; 16],
    };
    let auth = AuthenticatorRef {
        authenticator_id: vec![5; 16],
        device_epoch: 7,
    };
    let frame = ProviderFrame {
        command_id: vec![9; 16],
        operation_id: vec![8; 16],
        ..Default::default()
    };
    let mut key = vec![10; 16];
    key.extend([11; 16]);
    key.extend([5; 16]);
    let permit = MutationPermit {
        lease_id: vec![6; 16],
        scope: key,
        mutation_id: vec![13; 16],
    };
    assert!(valid_mutation(
        &context,
        &scope,
        &permit,
        &frame,
        &[13; 16],
        Some(&subject),
        Some(&auth)
    ));
    let mut wrong = permit.clone();
    wrong.lease_id[0] ^= 1;
    assert!(!valid_mutation(
        &context,
        &scope,
        &wrong,
        &frame,
        &[13; 16],
        Some(&subject),
        Some(&auth)
    ));
    let mut wrong = permit.clone();
    wrong.scope[16] ^= 1;
    assert!(!valid_mutation(
        &context,
        &scope,
        &wrong,
        &frame,
        &[13; 16],
        Some(&subject),
        Some(&auth)
    ));
    assert!(!valid_mutation(
        &context,
        &scope,
        &permit,
        &frame,
        &[14; 16],
        Some(&subject),
        Some(&auth)
    ));
    scope.claims[0].mode = 2;
    assert!(!valid_mutation(
        &context,
        &scope,
        &permit,
        &frame,
        &[13; 16],
        Some(&subject),
        Some(&auth)
    ));
}
