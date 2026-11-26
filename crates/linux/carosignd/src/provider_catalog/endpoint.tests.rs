use super::*;

#[test]
fn inventory_checks_correlation_and_identity_and_cannot_admit_work() {
    let request = [9; 16];
    let device = carosign_core::ids::AuthenticatorId::from_bytes([7; 16]).unwrap();
    let mut response = wire::Response {
        version: 1,
        request_id: request.to_vec(),
        result: Some(wire::response::Result::Inventory(wire::Inventory {
            authenticators: vec![AuthenticatorDescriptor {
                authenticator_id: device.as_bytes().to_vec(),
                device_epoch: 77,
                available: true,
                unavailability_reason: 0,
                capabilities: None,
            }],
        })),
    };
    let descriptor = decode_inventory(&response.encode_to_vec(), &request, |id| id == device)
        .unwrap()
        .remove(0);
    assert!(descriptor.available);
    assert_ne!(descriptor.device_epoch, 77);
    assert!(decode_inventory(&response.encode_to_vec(), &[8; 16], |id| id == device).is_err());
    let foreign = carosign_core::ids::AuthenticatorId::from_bytes([8; 16]).unwrap();
    assert!(decode_inventory(&response.encode_to_vec(), &request, |id| id == foreign).is_err());
    response.version = 2;
    assert!(decode_inventory(&response.encode_to_vec(), &request, |id| id == device).is_err());
}

#[test]
fn provider_assurance_survives_inventory_and_public_encoding() {
    use carosign_protocol::carosign::{
        client::v1::Authenticators,
        common::v1::{
            CapabilityDescriptor, DeclaredAssurance, PresentationAttackDetection,
            PresentationAttackDetectionSupport as Support, PresentationAttackType as Attack,
        },
    };
    let request = [9; 16];
    let device = carosign_core::ids::AuthenticatorId::from_bytes([7; 16]).unwrap();
    for (support, valid) in [(Support::Declared, true), (Support::Unsupported, false)] {
        let assurance = DeclaredAssurance {
            presentation_attack_detection: Some(PresentationAttackDetection {
                support: support as i32,
                covered_attacks: vec![Attack::PrintedPhoto as i32],
            }),
        };
        let response = wire::Response {
            version: 1,
            request_id: request.to_vec(),
            result: Some(wire::response::Result::Inventory(wire::Inventory {
                authenticators: vec![AuthenticatorDescriptor {
                    authenticator_id: device.as_bytes().to_vec(),
                    capabilities: Some(CapabilityDescriptor {
                        assurance: Some(assurance.clone()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
            })),
        };
        let result = decode_inventory(&response.encode_to_vec(), &request, |id| id == device);
        if valid {
            let public = Authenticators {
                providers: None,
                entries: result.unwrap(),
            };
            let decoded = Authenticators::decode(public.encode_to_vec().as_slice()).unwrap();
            assert_eq!(
                decoded.entries[0].capabilities.as_ref().unwrap().assurance,
                Some(assurance),
            );
        } else {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
        }
    }
}

#[test]
fn inventory_accepts_multiple_allowed_devices_and_rejects_duplicates() {
    let request = [9; 16];
    let first = AuthenticatorId::from_bytes([7; 16]).unwrap();
    let second = AuthenticatorId::from_bytes([8; 16]).unwrap();
    let mut inventory = wire::Inventory {
        authenticators: [first, second]
            .map(|id| AuthenticatorDescriptor {
                authenticator_id: id.as_bytes().to_vec(),
                device_epoch: 77,
                ..Default::default()
            })
            .into(),
    };
    let encode = |inventory: &wire::Inventory| {
        wire::Response {
            version: 1,
            request_id: request.to_vec(),
            result: Some(wire::response::Result::Inventory(inventory.clone())),
        }
        .encode_to_vec()
    };
    let allowed = |id| [first, second].contains(&id);
    let descriptors = decode_inventory(&encode(&inventory), &request, allowed).unwrap();
    assert_eq!(descriptors.len(), 2);
    assert!(
        descriptors
            .iter()
            .all(|descriptor| descriptor.device_epoch == 0)
    );
    assert!(decode_inventory(&encode(&inventory), &request, |id| id == first).is_err());
    inventory.authenticators[1].unavailability_reason = 1;
    assert!(decode_inventory(&encode(&inventory), &request, allowed).is_err());
    inventory.authenticators[1].unavailability_reason = 0;
    inventory.authenticators[1].authenticator_id = first.as_bytes().to_vec();
    assert!(decode_inventory(&encode(&inventory), &request, allowed).is_err());
}
