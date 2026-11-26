use super::*;

#[test]
fn only_bind_carries_descriptors() {
    assert!(may_carry_descriptors("BindAuthenticator"));
    assert!(!may_carry_descriptors("StartVerification"));
    assert!(!may_carry_descriptors("WorkerHello"));
}
