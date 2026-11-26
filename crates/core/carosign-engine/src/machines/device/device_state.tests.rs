use super::*;

#[test]
fn disconnect_makes_every_device_state_unavailable() {
    for state in [
        DeviceState::Available,
        DeviceState::Reserved,
        DeviceState::Active,
        DeviceState::Quiescing,
        DeviceState::Uncertain,
        DeviceState::Recovering,
        DeviceState::Unavailable,
    ] {
        let next = next_state(state, DeviceEvent::Disconnected);
        assert_eq!(next, Some(DeviceState::Unavailable));
        assert!(!can_reserve(next.unwrap()));
    }
}

#[test]
fn fault_after_disconnect_cannot_erase_unavailability() {
    let state = next_state(DeviceState::Reserved, DeviceEvent::Disconnected).unwrap();
    let state = next_state(state, DeviceEvent::Fault).unwrap();
    assert_eq!(state, DeviceState::Unavailable);
    assert_eq!(next_state(state, DeviceEvent::Quiesced), None);
    assert_eq!(next_state(state, DeviceEvent::Recover), None);
    let reconnected = next_state(state, DeviceEvent::Reconnected).unwrap();
    assert!(!can_reserve(reconnected));
    let recovering = next_state(reconnected, DeviceEvent::Recover).unwrap();
    assert!(!can_reserve(recovering));
    assert_eq!(
        next_state(recovering, DeviceEvent::Quiesced),
        Some(DeviceState::Available)
    );
}

#[test]
fn reconnect_does_not_itself_authorize_device_use() {
    assert_eq!(
        next_state(DeviceState::Unavailable, DeviceEvent::Quiesced),
        None
    );
    assert_eq!(
        next_state(DeviceState::Unavailable, DeviceEvent::Recover),
        None
    );
    let state = next_state(DeviceState::Unavailable, DeviceEvent::Reconnected).unwrap();
    assert_eq!(state, DeviceState::Uncertain);
    assert!(!can_reserve(state));
    assert_eq!(next_state(state, DeviceEvent::Quiesced), None);
    assert_eq!(
        next_state(DeviceState::Available, DeviceEvent::Reconnected),
        None
    );
}

#[test]
fn only_available_device_can_be_reserved_again() {
    for state in [
        DeviceState::Reserved,
        DeviceState::Active,
        DeviceState::Quiescing,
        DeviceState::Uncertain,
        DeviceState::Recovering,
        DeviceState::Unavailable,
    ] {
        assert!(!can_reserve(state));
        assert_eq!(next_state(state, DeviceEvent::Reserved), None);
    }
    assert!(can_reserve(DeviceState::Available));
    assert_eq!(
        next_state(DeviceState::Available, DeviceEvent::Reserved),
        Some(DeviceState::Reserved)
    );
}
