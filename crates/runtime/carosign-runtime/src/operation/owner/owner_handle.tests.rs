use super::super::*;
use crate::ports::CallerContext;
use crate::ports::TransactionId;

#[test]
fn detached_owner_gets_nothing() {
    let mut owner = OwnerHandle::new(CallerContext::from_transport(
        [1u8; 32],
        1000,
        TransactionId::from_bytes([7u8; 16]).expect("ненулевой"),
    ));
    assert!(owner.may_receive());
    owner.detach();
    assert!(!owner.may_receive());
}
