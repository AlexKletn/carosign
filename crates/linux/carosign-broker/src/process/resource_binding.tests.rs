use super::*;
use carosign_protocol::carosign::{
    broker_relay::v1::{AccessMode, Claim, Owner, Slot},
    common::v1::AuthenticatorRef,
    provider::v1::{BindAuthenticator, FdRole, FdSlot},
    resource_owner::v1 as wire,
};
use carosign_protocol::resource_owner::Quiescence;
use prost::Message;
use rustix::net::{RecvFlags, SendFlags, recv, send};

const RESOURCE: [u8; 16] = [7; 16];
const CONFIG: [u8; 16] = [8; 16];
const REQUEST: [u8; 16] = [9; 16];

fn context() -> Context {
    Context {
        worker: [1; 16],
        epoch: 2,
        broker: [3; 16],
        boot: [4; 16],
    }
}

fn bind_request(resource_id: [u8; 16], with_config: bool) -> Bind {
    let ctx = context();
    let owner = Owner {
        operation_id: vec![5; 16],
        worker_instance: ctx.worker.to_vec(),
        authenticator_id: vec![6; 16],
        device_epoch: ctx.epoch,
    };
    let mut claims = vec![Claim {
        resource_id: resource_id.to_vec(),
        mode: AccessMode::Exclusive as i32,
    }];
    let mut slots = Vec::new();
    let mut fd_slots = Vec::new();
    if with_config {
        claims.push(Claim {
            resource_id: CONFIG.to_vec(),
            mode: AccessMode::Exclusive as i32,
        });
        slots.push(Slot {
            resource_id: CONFIG.to_vec(),
            role: FdRole::Config as i32,
        });
        fd_slots.push(FdSlot {
            index: 0,
            resource_id: CONFIG.to_vec(),
            role: FdRole::Config as i32,
        });
    }
    let body = BindAuthenticator {
        authenticator: Some(AuthenticatorRef {
            authenticator_id: owner.authenticator_id.clone(),
            device_epoch: owner.device_epoch,
        }),
        profile_id: vec![10; 16],
        lease_id: vec![11; 16],
        fd_slots,
    };
    Bind {
        request_id: REQUEST.to_vec(),
        scope: Some(Scope {
            lease_id: body.lease_id.clone(),
            owner: Some(owner.clone()),
            claims,
        }),
        slots,
        frame: Some(ProviderFrame {
            operation_id: owner.operation_id,
            payload: Some(Payload::BindAuthenticator(body)),
            ..Default::default()
        }),
    }
}

#[test]
fn owner_managed_prepare_and_quiescence_use_the_same_scope_without_fds() {
    for wrong_scope in [false, true] {
        let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let input = bind_request(RESOURCE, false);
        let expected = input.scope.clone();
        let owner = std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            let (bytes, _) = recv(&sockets.worker, &mut buffer[..], RecvFlags::empty()).unwrap();
            let prepare = wire::Prepare::decode(&buffer[..bytes]).unwrap();
            assert_eq!(prepare.scope, expected);
            assert!(prepare.slots.is_empty());
            let response = wire::Response {
                request_id: prepare.request_id,
                boot_id: prepare.boot_id,
                result: Some(wire::response::Result::Prepared(wire::Prepared {
                    scope: prepare.scope.clone(),
                    slots: Vec::new(),
                })),
            };
            send(
                &sockets.worker,
                &response.encode_to_vec(),
                SendFlags::empty(),
            )
            .unwrap();

            let (bytes, _) = recv(&sockets.worker, &mut buffer[..], RecvFlags::empty()).unwrap();
            let inspect = wire::InspectQuiescence::decode(&buffer[..bytes]).unwrap();
            assert_eq!(inspect.scope, prepare.scope);
            let mut scope = inspect.scope.unwrap();
            if wrong_scope {
                scope.lease_id[0] ^= 1;
            }
            let response = wire::QuiescenceObservation {
                request_id: inspect.request_id,
                boot_id: inspect.boot_id,
                scope: Some(scope),
                stage: wire::QuiescenceStage::QuiescenceExclusiveQuiesced as i32,
            };
            send(
                &sockets.worker,
                &response.encode_to_vec(),
                SendFlags::empty(),
            )
            .unwrap();
        });

        let mut binding =
            ResourceBinding::new(Vec::new(), Some(sockets.platform), Some(RESOURCE)).unwrap();
        assert!(!binding.allows_prebind());
        let (_, _, fds) = binding
            .bind(&context(), Some(&[10; 16]), REQUEST, input)
            .unwrap();
        assert!(fds.is_empty());
        let outcome = binding
            .owner_session
            .as_mut()
            .unwrap()
            .inspect_quiescence([12; 16], Duration::from_secs(2));
        owner.join().unwrap();
        if wrong_scope {
            assert!(outcome.is_err());
        } else {
            assert_eq!(outcome.unwrap(), Quiescence::ExclusiveQuiesced);
        }
    }
}

#[test]
fn owner_managed_resource_requires_configured_exclusive_claim() {
    let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let mut binding =
        ResourceBinding::new(Vec::new(), Some(sockets.platform), Some(RESOURCE)).unwrap();
    let foreign = bind_request([1; 16], false);
    let mut shared = bind_request(RESOURCE, false);
    shared.scope.as_mut().unwrap().claims[0].mode = AccessMode::SharedRead as i32;
    for request in [foreign, shared] {
        assert!(
            binding
                .bind(&context(), Some(&[10; 16]), REQUEST, request)
                .is_err()
        );
    }
    assert!(binding.owner_channel.is_some());
    assert!(ResourceBinding::new(Vec::new(), None, Some(RESOURCE)).is_err());
}
