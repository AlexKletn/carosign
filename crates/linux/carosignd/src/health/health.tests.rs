use super::*;

fn ready() -> HealthSnapshot {
    HealthSnapshot {
        startup_complete: true,
        listener_published: true,
        policy_ready: true,
        available_authenticators: 2,
        unavailable_authenticators: 3,
        scopes_needing_reconciliation: 0,
    }
}

#[test]
fn readiness_requires_all_evidence_and_no_closed_scope() {
    assert!(ready().into_wire().service_ready);
    let mut value = ready();
    value.startup_complete = false;
    assert!(!value.into_wire().service_ready);
    let mut value = ready();
    value.listener_published = false;
    assert!(!value.into_wire().service_ready);
    let mut value = ready();
    value.policy_ready = false;
    assert!(!value.into_wire().service_ready);
    let mut value = ready();
    value.scopes_needing_reconciliation = 1;
    let wire = value.into_wire();
    assert!(!wire.service_ready);
    assert_eq!(wire.scopes_needing_reconciliation, 1);
    assert!(UnavailableHealth.snapshot().is_err());
}

#[test]
fn zero_authenticators_do_not_make_ready_service_unhealthy() {
    let mut value = ready();
    value.available_authenticators = 0;
    value.unavailable_authenticators = 0;
    let wire = value.into_wire();
    assert!(wire.service_ready);
    assert_eq!(wire.available_authenticators, 0);
    assert_eq!(wire.unavailable_authenticators, 0);
}
