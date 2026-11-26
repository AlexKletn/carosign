#[test]
#[ignore = "требует broker и mock в сценарии linux-lost-rollback"]
fn mutation_lost_rollback_keeps_scope_for_reconciliation() {
    super::cancel_enrollment(false);
}

#[test]
#[ignore = "требует broker и mock в сценарии linux-pending-rollback"]
fn mutation_confirmed_rollback_releases_scope() {
    super::cancel_enrollment(true);
}

#[test]
#[ignore = "требует broker и mock в сценарии linux-error-rollback"]
fn mutation_provider_error_rolls_back_and_allows_retry() {
    super::exercise_enrollment_rollback(true, false, false);
}

#[test]
#[ignore = "требует broker и mock в сценарии linux-committed-rollback"]
fn mutation_committed_before_rollback_keeps_scope_for_reconciliation() {
    super::cancel_enrollment(false);
}

#[test]
#[ignore = "требует broker и mock в сценарии linux-pending-rollback"]
fn system_client_cancel_uses_live_rollback_and_releases_scope() {
    super::cancel_enrollment_via(true, true);
}
