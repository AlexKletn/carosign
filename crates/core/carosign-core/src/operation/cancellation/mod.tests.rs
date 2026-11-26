use super::*;

#[test]
fn receipt_is_not_quiescence() {
    assert!(!CancelReceipt::Requested.terminal_by_receipt_alone());
    assert!(CancelReceipt::AlreadyTerminal.terminal_by_receipt_alone());
}
