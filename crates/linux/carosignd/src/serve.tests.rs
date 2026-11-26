#[path = "serve_tests_fixtures/broken_clock.rs"]
mod broken_clock;
#[path = "serve_tests_fixtures/clock.rs"]
mod clock;
#[path = "serve_tests_fixtures/ids.rs"]
mod ids;

use super::*;
use carosign_core::ids::{AuthenticatorId, AuthorityId, BootId, GrantId, OperationId, SubjectId};
use carosign_core::operation::context::BootInstant;

use ids::Ids;

use clock::Clock;

fn request() -> Request {
    Request {
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
            subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
        },
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
            device_epoch: carosign_core::device::DeviceEpoch::FIRST,
        },
        purpose: VerificationPurpose::SessionLogin,
        owner: OwnerRef {
            authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
            binding: GrantId::from_bytes([4u8; 16]).expect("валид"),
        },
        deadline_ns: 1_000_000,
    }
}

#[test]
fn context_gets_a_fresh_id_and_boot_bound_deadline() {
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let first = request().context(&ids, &Clock).unwrap();
    let second = request().context(&ids, &Clock).unwrap();
    // ⚠️ Two orders, two numbers.
    assert_ne!(first.operation_id, second.operation_id);
    assert_eq!(first.deadline.boot(), Clock.boot_id());
}

#[test]
fn inconsistent_clock_or_deadline_overflow_yields_expired_context() {
    use broken_clock::BrokenClock;
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let boot = BootId::from_bytes([8; 16]).unwrap();
    for clock in [
        BrokenClock {
            now: BootInstant::new(boot, 10),
            boot: BootId::from_bytes([9; 16]).unwrap(),
        },
        BrokenClock {
            now: BootInstant::new(boot, u64::MAX - 1),
            boot,
        },
    ] {
        let context = request().context(&ids, &clock).unwrap();
        assert!(context.deadline.is_expired_at(clock.now()));
    }
}

#[test]
fn actor_starts_in_new_and_takes_admission() {
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let context = request().context(&ids, &Clock).unwrap();
    let mut actor = actor_for(&request(), context);
    assert_eq!(
        actor.state().common.phase,
        carosign_engine::OperationPhase::New
    );
    admit(&mut actor).expect("допуск принят");
    // ⚠️ Admission is a **message**, not a transition: a transition takes a step.
    assert_eq!(
        actor.state().common.phase,
        carosign_engine::OperationPhase::New
    );
    let step = carosign_runtime::step_actor(
        &mut actor,
        carosign_runtime::OperationMsg::Domain(carosign_engine::DomainInput::Progress),
    );
    let _ = step;
    assert_eq!(
        actor.state().common.phase,
        carosign_engine::OperationPhase::WaitingLease
    );
}
