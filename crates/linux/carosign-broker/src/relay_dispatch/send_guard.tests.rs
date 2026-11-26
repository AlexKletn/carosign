use super::*;
use carosign_protocol::carosign::provider::v1 as provider;

#[test]
fn bind_and_cancel_cannot_be_normal_commands() {
    assert_eq!(
        direction(&Payload::BindAuthenticator(
            provider::BindAuthenticator::default()
        )),
        Direction::Bind
    );
    assert_eq!(
        direction(&Payload::RequestCancel(provider::RequestCancel::default())),
        Direction::BrokerCancel
    );
    assert_eq!(
        direction(&Payload::Initialize(provider::Initialize::default())),
        Direction::Command
    );
    assert_eq!(
        direction(&Payload::WorkerHello(provider::WorkerHello::default())),
        Direction::Event
    );
}

#[test]
fn revoke_before_send_prevents_begin() {
    let mut guard = SendGuard::new();
    guard.revoke().unwrap();
    assert_eq!(guard.begin(), Err(InvalidTransition));
    assert_eq!(guard.state(), SendState::Revoked);
}

#[test]
fn possible_send_is_never_retried_after_revoke_or_loss() {
    for sent in [false, true] {
        for revoke in [false, true] {
            let mut guard = SendGuard::new();
            guard.begin().unwrap();
            if sent {
                guard.sent().unwrap();
            }
            if revoke {
                guard.revoke().unwrap();
            } else {
                guard.lost().unwrap();
            }
            assert_eq!(guard.state(), SendState::Uncertain);
            assert_eq!(guard.begin(), Err(InvalidTransition));
            assert_eq!(guard.sent(), Err(InvalidTransition));
        }
    }
}

#[test]
fn eagain_preserves_pending_send_and_allows_revoke_before_retry() {
    let mut guard = SendGuard::new();
    guard.begin().unwrap();
    guard.would_block().unwrap();
    guard.revoke().unwrap();
    assert_eq!(guard.begin(), Err(InvalidTransition));
    assert_eq!(guard.state(), SendState::Revoked);
}
