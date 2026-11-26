use super::*;

#[test]
fn unbound_epoch_never_authorizes_a_device_command() {
    let command = Command::RequestCancel {
        target: vec![1; 16],
    };
    assert_eq!(admittance(&command, 0, 0), Action::Refuse);
    assert_eq!(admittance(&command, 1, 0), Action::Refuse);
    assert_eq!(admittance(&command, 1, 2), Action::Refuse);
    assert_eq!(admittance(&command, 1, 1), Action::Run);
    assert_eq!(admittance(&Command::Shutdown, 0, 0), Action::Run);
}

#[test]
fn authenticator_epoch_must_match_the_frame_and_binding() {
    let mut command = Command::Describe {
        authenticator: carosign_protocol::carosign::common::v1::AuthenticatorRef {
            authenticator_id: vec![1; 16],
            device_epoch: 2,
        },
    };
    assert_eq!(admittance(&command, 1, 1), Action::Refuse);
    if let Command::Describe { authenticator } = &mut command {
        authenticator.device_epoch = 1;
    }
    assert_eq!(admittance(&command, 1, 1), Action::Run);
    if let Command::Describe { authenticator } = &mut command {
        authenticator.authenticator_id = vec![0; 16];
    }
    assert_eq!(admittance(&command, 1, 1), Action::Refuse);
}
