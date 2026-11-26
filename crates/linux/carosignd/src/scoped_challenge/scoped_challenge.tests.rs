use super::*;
use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, AuthorityId, BootId, SubjectId};
use carosign_core::operation::context::{BootDeadline, VerificationPurpose};
use carosign_evidence::recipient::{ChallengeRef, RecipientRef};

fn context() -> (OperationId, ScopedAssertionContext, BootInstant) {
    let boot = BootId::from_bytes([1; 16]).unwrap();
    (
        OperationId::from_bytes([2; 16]).unwrap(),
        ScopedAssertionContext {
            subject: SubjectRef {
                authority: AuthorityId::from_bytes([3; 16]).unwrap(),
                subject: SubjectId::from_bytes([4; 16]).unwrap(),
            },
            authenticator: AuthenticatorRef {
                id: AuthenticatorId::from_bytes([5; 16]).unwrap(),
                device_epoch: DeviceEpoch::FIRST,
            },
            purpose: VerificationPurpose::SessionLogin,
            recipient: RecipientRef::new([6; 16]).unwrap(),
            challenge: ChallengeRef::accepted_by_recipient([7; 16]).unwrap(),
            deadline: BootDeadline::new(boot, 100),
        },
        BootInstant::new(boot, 10),
    )
}

#[test]
fn concurrent_consumption_accepts_exactly_one_and_keeps_tombstone() {
    let ledger = std::sync::Arc::new(ScopedChallenges::new(1));
    let (operation, context, now) = context();
    ledger.register(operation, context.clone(), now).unwrap();
    let mut foreign = context.clone();
    foreign.recipient = RecipientRef::new([8; 16]).unwrap();
    assert_eq!(
        ledger.consume(operation, &foreign, now),
        Err(ChallengeError::Mismatch)
    );
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let ledger = ledger.clone();
            let barrier = barrier.clone();
            let context = context.clone();
            std::thread::spawn(move || {
                barrier.wait();
                ledger.consume(operation, &context, now)
            })
        })
        .collect();
    barrier.wait();
    let outcomes: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|x| x.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|x| **x == Err(ChallengeError::Consumed))
            .count(),
        1
    );
    assert_eq!(
        ledger.register(operation, context, now),
        Err(ChallengeError::Duplicate)
    );
}

#[test]
fn expiration_cannot_be_undone_by_clock_regression() {
    let ledger = ScopedChallenges::new(1);
    let (operation, context, now) = context();
    ledger.register(operation, context.clone(), now).unwrap();
    let expired = BootInstant::new(context.deadline.boot(), 100);
    assert_eq!(
        ledger.consume(operation, &context, expired),
        Err(ChallengeError::Expired)
    );
    assert_eq!(
        ledger.consume(operation, &context, now),
        Err(ChallengeError::Consumed)
    );
}

#[test]
fn wrong_operation_and_full_table_do_not_create_authority() {
    let ledger = ScopedChallenges::new(1);
    let (operation, context, now) = context();
    ledger.register(operation, context.clone(), now).unwrap();
    let foreign = OperationId::from_bytes([9; 16]).unwrap();
    assert_eq!(
        ledger.consume(foreign, &context, now),
        Err(ChallengeError::Mismatch)
    );
    let mut another = context.clone();
    another.challenge = ChallengeRef::accepted_by_recipient([10; 16]).unwrap();
    assert_eq!(
        ledger.register(operation, another, now),
        Err(ChallengeError::Full)
    );
    assert_eq!(ledger.consume(operation, &context, now), Ok(()));
}

#[test]
fn issued_random_challenge_is_bound_and_single_use() {
    let ledger = ScopedChallenges::new(2);
    let (operation, context, now) = context();
    let issued = ledger.issue(operation, context, now).unwrap();
    assert_ne!(issued.challenge.id(), [0; 16]);
    assert_eq!(ledger.consume(operation, &issued, now), Ok(()));
    assert_eq!(
        ledger.consume(operation, &issued, now),
        Err(ChallengeError::Consumed)
    );
}
