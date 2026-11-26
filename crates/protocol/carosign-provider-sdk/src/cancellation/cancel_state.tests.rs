use super::*;

#[test]
fn receipt_is_not_quiescence() {
    let requested = CancelState::Idle.request();
    assert_eq!(requested, CancelState::Requested);
    assert!(!requested.is_quiesced());
    assert!(requested.confirm_quiesced().is_quiesced());
}

#[test]
fn unsupported_stays_unsupported() {
    assert_eq!(CancelState::Unsupported.request(), CancelState::Unsupported);
}

#[test]
fn quiescence_is_not_an_already_delivered_terminal_outcome() {
    let state = CancelState::Idle.request().confirm_quiesced();
    assert_eq!(state.receipt(), provider::CancelStatus::CancelRequested);
    assert!(state.request().is_quiesced());
}
