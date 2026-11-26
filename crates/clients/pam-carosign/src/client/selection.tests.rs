use super::super::*;
use carosign_protocol::carosign::common::v1 as common;

#[test]
fn multiple_devices_require_a_unique_enrolled_candidate() {
    let device = |id| common::AuthenticatorDescriptor {
        authenticator_id: vec![id; 16],
        device_epoch: 1,
        available: true,
        unavailability_reason: 0,
        capabilities: Some(common::CapabilityDescriptor {
            operations: vec![common::ProviderOperation::ProviderVerify as i32],
            ..Default::default()
        }),
    };
    let devices = [device(1), device(2)];
    assert_eq!(
        select_for_account(&devices, |id| Ok(id[0] == 2)).unwrap(),
        Some(vec![2; 16])
    );
    assert_eq!(select_for_account(&devices, |_| Ok(true)).unwrap(), None);
    assert_eq!(select_for_account(&devices, |_| Ok(false)).unwrap(), None);
    assert!(
        select_for_account(&devices, |_| Err(
            carosign_linux_ipc::client::transport::TransportError::Unavailable
        ))
        .is_err()
    );
}

#[test]
fn selects_only_available_verification_device() {
    let mut device = common::AuthenticatorDescriptor {
        authenticator_id: vec![3; 16],
        device_epoch: 1,
        available: false,
        unavailability_reason: 0,
        capabilities: Some(common::CapabilityDescriptor {
            operations: vec![common::ProviderOperation::ProviderVerify as i32],
            ..Default::default()
        }),
    };
    assert!(select_authenticator(&[device.clone()]).is_none());
    device.available = true;
    assert_eq!(select_authenticator(&[device.clone()]), Some(vec![3; 16]));
    device.capabilities.as_mut().unwrap().operations.clear();
    assert!(select_authenticator(&[device]).is_none());
}
