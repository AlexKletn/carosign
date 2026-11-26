use crate::carosign::{broker_relay::v1 as relay, provider::v1::FdRole, resource_owner::v1 as owner};

use super::*;

fn request() -> owner::Prepare {
    owner::Prepare {
        request_id: vec![1; 16],
        boot_id: vec![2; 16],
        scope: Some(relay::Scope {
            lease_id: vec![3; 16],
            owner: Some(relay::Owner {
                operation_id: vec![4; 16],
                worker_instance: vec![5; 16],
                authenticator_id: vec![6; 16],
                device_epoch: 1,
            }),
            claims: vec![
                relay::Claim {
                    resource_id: vec![7; 16],
                    mode: relay::AccessMode::Exclusive as i32,
                },
                relay::Claim {
                    resource_id: vec![8; 16],
                    mode: relay::AccessMode::Exclusive as i32,
                },
            ],
        }),
        slots: vec![
            relay::Slot {
                resource_id: vec![7; 16],
                role: FdRole::Device as i32,
            },
            relay::Slot {
                resource_id: vec![8; 16],
                role: FdRole::Device as i32,
            },
        ],
    }
}

#[test]
fn matches_scope_order_and_exact_fd_count() {
    let input = request();
    let mut response = owner::Response {
        request_id: input.request_id.clone(),
        boot_id: input.boot_id.clone(),
        result: Some(owner::response::Result::Prepared(owner::Prepared {
            scope: input.scope.clone(),
            slots: input.slots.clone(),
        })),
    };
    assert_eq!(
        validate_response(&input, &response, 2),
        Ok(Outcome::Prepared)
    );
    assert_eq!(
        validate_response(&input, &response, 1),
        Err(Error::InvalidDescriptors)
    );
    if let Some(owner::response::Result::Prepared(ready)) = &mut response.result {
        ready.slots.swap(0, 1);
    }
    assert_eq!(
        validate_response(&input, &response, 2),
        Err(Error::MismatchedResponse)
    );
    response.boot_id[0] ^= 1;
    assert_eq!(
        validate_response(&input, &response, 2),
        Err(Error::MismatchedResponse)
    );
}

#[test]
fn rejects_foreign_duplicate_and_nonexclusive_claims() {
    let mut input = request();
    input.slots[1].resource_id = vec![9; 16];
    assert_eq!(validate_prepare(&input), Err(Error::InvalidRequest));
    input = request();
    input.slots[1].resource_id = input.slots[0].resource_id.clone();
    assert_eq!(validate_prepare(&input), Err(Error::InvalidRequest));
    input = request();
    input.scope.as_mut().unwrap().claims[0].mode = relay::AccessMode::SharedRead as i32;
    assert_eq!(validate_prepare(&input), Err(Error::InvalidRequest));
}

#[test]
fn refused_response_never_accepts_fds() {
    let input = request();
    let response = owner::Response {
        request_id: input.request_id.clone(),
        boot_id: input.boot_id.clone(),
        result: Some(owner::response::Result::Refused(
            crate::carosign::common::v1::Error {
                code: 1,
                safe_message: String::new(),
            },
        )),
    };
    assert_eq!(
        validate_response(&input, &response, 0),
        Ok(Outcome::Refused)
    );
    assert_eq!(
        validate_response(&input, &response, 1),
        Err(Error::InvalidDescriptors)
    );
}

#[test]
fn quiescence_requires_original_scope_new_id_and_no_fds() {
    let prepare = request();
    let mut inspect = owner::InspectQuiescence {
        request_id: vec![9; 16],
        boot_id: prepare.boot_id.clone(),
        scope: prepare.scope.clone(),
    };
    let mut response = owner::QuiescenceObservation {
        request_id: inspect.request_id.clone(),
        boot_id: inspect.boot_id.clone(),
        scope: inspect.scope.clone(),
        stage: 1,
    };
    assert_eq!(
        validate_quiescence_observation(&prepare, &inspect, &response, 0),
        Ok(Quiescence::Draining)
    );
    assert_eq!(
        validate_quiescence_observation(&prepare, &inspect, &response, 1),
        Err(Error::InvalidDescriptors)
    );
    response.stage = 2;
    assert_eq!(
        validate_quiescence_observation(&prepare, &inspect, &response, 0),
        Ok(Quiescence::HardwareUnconfirmed)
    );
    response.stage = 3;
    assert_eq!(
        validate_quiescence_observation(&prepare, &inspect, &response, 0),
        Ok(Quiescence::Unknown)
    );
    response.stage = 4;
    assert_eq!(
        validate_quiescence_observation(&prepare, &inspect, &response, 0),
        Ok(Quiescence::LeaseRevoked)
    );
    response.stage = 5;
    assert_eq!(
        validate_quiescence_observation(&prepare, &inspect, &response, 0),
        Ok(Quiescence::ExclusiveLeaseRevoked)
    );
    response.stage = 6;
    assert_eq!(
        validate_quiescence_observation(&prepare, &inspect, &response, 0),
        Ok(Quiescence::ExclusiveQuiesced)
    );
    response.stage = 7;
    assert_eq!(
        validate_quiescence_observation(&prepare, &inspect, &response, 0),
        Err(Error::MismatchedResponse)
    );
    inspect.request_id = prepare.request_id.clone();
    assert_eq!(
        validate_quiescence_request(&prepare, &inspect),
        Err(Error::InvalidRequest)
    );
    inspect.request_id = vec![9; 16];
    inspect
        .scope
        .as_mut()
        .unwrap()
        .owner
        .as_mut()
        .unwrap()
        .device_epoch += 1;
    assert_eq!(
        validate_quiescence_request(&prepare, &inspect),
        Err(Error::InvalidRequest)
    );
}
