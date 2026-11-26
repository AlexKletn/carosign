use super::*;

#[test]
fn needs_reconciliation_keeps_scope() {
    assert!(!MutationClosure::NeedsReconciliation.releases_scope());
    assert!(MutationClosure::Completed.releases_scope());
    assert!(MutationClosure::NoChange.releases_scope());
}
