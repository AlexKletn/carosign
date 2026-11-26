//! B-04 and B-06: Same request and where does the right come from?
//!
//! ⚠️ The main thing is checked: the “same request” feature is no longer the function
//! argument, but the transaction number in the context; the right is taken from the policy
//! file, and the unfamiliar field in it is a refusal.

use carosign_adapters::clock::StepClock;
use carosign_adapters::policy::{Action, FileIdentity, FilePolicy, parse};
use carosign_core::authorization::GrantScope;
use carosign_core::identity::{SubjectRef, SubjectSelector};
use carosign_core::ids::{AuthorityId, BootId, SubjectId};
use carosign_runtime::ports::{
    AuthorityRefusal, CallerContext, IdentityAuthority, PolicyAuthority, TransactionId,
};

const POLICY: &str = r#"
# проверка и регистрация своему
uid=1000 subject=42 actions=verify,enroll lifetime=30
# чужому нельзя ничего
uid=1001 subject=7 actions=observe
"#;

#[test]
fn policy_errors_do_not_log_untrusted_keys_or_actions() {
    for input in [
        "uid=1000 subject=42 actions=verify secret-marker\u{1b}=value",
        "uid=1000 subject=42 actions=secret-marker\u{1b}",
    ] {
        let error = parse(input).unwrap_err();
        for output in [format!("{error}"), format!("{error:?}")] {
            assert!(!output.contains("secret-marker"));
            assert!(!output.contains('\u{1b}'));
        }
    }
}

fn caller(uid: u32, transaction: u8) -> CallerContext {
    // ⚠️ uid is a context field, not a tag byte.
    CallerContext::from_transport(
        [0u8; 32],
        uid,
        TransactionId::from_bytes([transaction; 16]).expect("ненулевой"),
    )
}

fn authority() -> AuthorityId {
    AuthorityId::from_bytes([1u8; 16]).expect("валид")
}

fn clock() -> StepClock {
    StepClock::new(BootId::from_bytes([9; 16]).unwrap())
}

fn subject(subject: u64) -> SubjectRef {
    let mut bytes = [0u8; 16];
    bytes[8..].copy_from_slice(&subject.to_be_bytes());
    bytes[0] = 1;
    SubjectRef {
        authority: authority(),
        subject: SubjectId::from_bytes(bytes).expect("ненулевой"),
    }
}

fn verify_scope(subject: SubjectRef) -> GrantScope {
    GrantScope::Verify {
        subject,
        authenticator: carosign_core::ids::AuthenticatorId::from_bytes([6; 16]).unwrap(),
        purpose: carosign_core::operation::context::VerificationPurpose::SessionLogin,
    }
}

#[test]
fn transaction_number_is_what_makes_it_the_same_request() {
    // B-04: A number in context, not a function argument.
    let first = caller(1000, 1);
    let second = caller(1000, 2);
    assert!(first.answers(first.transaction()));
    assert!(!first.answers(second.transaction()));
    assert_ne!(first, second);
}

#[test]
fn zero_transaction_is_not_a_transaction() {
    assert!(TransactionId::from_bytes([0u8; 16]).is_none());
}

#[test]
fn policy_grants_what_it_permits_only() {
    let policy = parse(POLICY).expect("политика");
    let clock = clock();
    let grants = FilePolicy::with_clock(&policy, authority(), &clock);

    let own = caller(1000, 1);
    let allowed = futures_executor::block_on(grants.authorize(&own, verify_scope(subject(42))));
    assert!(allowed.is_ok());

    let stranger = caller(1001, 2);
    let denied = futures_executor::block_on(grants.authorize(&stranger, verify_scope(subject(42))));
    assert_eq!(denied.unwrap_err(), AuthorityRefusal::Denied);
}

#[test]
fn file_policy_does_not_guess_the_subject_or_owner_of_an_operation() {
    let policy = parse("uid=1000 subject=42 actions=observe,cancel lifetime=30").unwrap();
    let clock = clock();
    let grants = FilePolicy::with_clock(&policy, authority(), &clock);
    let operation = carosign_core::ids::OperationId::from_bytes([7; 16]).unwrap();
    for scope in [
        GrantScope::Observe(operation),
        GrantScope::Cancel(operation),
    ] {
        assert_eq!(
            futures_executor::block_on(grants.authorize(&caller(1000, 1), scope)).unwrap_err(),
            AuthorityRefusal::Denied
        );
    }
}

#[test]
fn identity_comes_from_the_policy_not_the_request() {
    // B-06: Personality is taken from uid connection policy.
    let policy = parse(&format!(
        "{POLICY}\nuid=1000 subject=1000 actions=inventory\n"
    ))
    .expect("политика");
    let identity = FileIdentity::new(&policy, authority());

    let known =
        futures_executor::block_on(identity.resolve(&caller(1000, 1), &SubjectSelector::Current));
    assert_eq!(known.expect("найден"), subject(42));

    let unknown =
        futures_executor::block_on(identity.resolve(&caller(4242, 1), &SubjectSelector::Current));
    assert!(unknown.is_err());
}

#[test]
fn foreign_or_noncanonical_subject_cannot_use_local_rule() {
    let policy = parse(POLICY).unwrap();
    let clock = clock();
    let grants = FilePolicy::with_clock(&policy, authority(), &clock);
    let mut foreign = subject(42);
    foreign.authority = AuthorityId::from_bytes([8; 16]).unwrap();
    let mut bytes = *subject(42).subject.as_bytes();
    bytes[1] = 1;
    let noncanonical = SubjectRef {
        authority: authority(),
        subject: SubjectId::from_bytes(bytes).unwrap(),
    };
    for reference in [foreign, noncanonical] {
        assert_eq!(
            futures_executor::block_on(grants.authorize(&caller(1000, 1), verify_scope(reference)))
                .unwrap_err(),
            AuthorityRefusal::Denied
        );
    }
}

#[test]
fn ambiguous_identity_or_unresolved_selector_is_refused() {
    let policy =
        parse("uid=1000 subject=42 actions=verify\nuid=1000 subject=43 actions=verify").unwrap();
    let identity = FileIdentity::new(&policy, authority());
    assert!(
        futures_executor::block_on(identity.resolve(&caller(1000, 1), &SubjectSelector::Current))
            .is_err()
    );
    let policy = parse(POLICY).unwrap();
    let identity = FileIdentity::new(&policy, authority());
    let selector =
        SubjectSelector::AccountHint(carosign_core::identity::AccountHint::new("other").unwrap());
    assert!(futures_executor::block_on(identity.resolve(&caller(1000, 1), &selector)).is_err());
}

#[test]
fn unknown_field_in_policy_is_refused() {
    // ⚠️ A typo should not expand rights.
    let bad = "uid=1000 subject=42 actions=verify admin=yes";
    assert!(parse(bad).is_err());
    let unknown_action = "uid=1000 subject=42 actions=verify,root";
    assert!(parse(unknown_action).is_err());
}

#[test]
fn repeated_policy_fields_are_rejected() {
    for text in [
        "uid=1000 uid=1001 subject=42 actions=verify",
        "uid=1000 subject=42 subject=43 actions=verify",
        "uid=1000 subject=42 actions=verify actions=enroll",
        "uid=1000 subject=42 actions=verify lifetime=30 lifetime=60",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
}

#[test]
fn revalidation_reads_policy_now() {
    // ⚠️ The revocation of the rule is effective immediately, not on the next issue.
    let policy = parse(POLICY).expect("политика");
    let clock = clock();
    let grants = FilePolicy::with_clock(&policy, authority(), &clock);
    let own = caller(1000, 1);
    let grant = futures_executor::block_on(grants.authorize(&own, verify_scope(subject(42))))
        .expect("выдано");

    assert!(
        futures_executor::block_on(grants.revalidate(
            &grant,
            carosign_core::authorization::AuthorizationBoundary::BeforeDispatch
        ))
        .is_ok()
    );

    // Politics without a rule - rechecking fails.
    let empty = parse("").expect("пустая");
    let revoked = FilePolicy::with_clock(&empty, authority(), &clock);
    assert_eq!(
        futures_executor::block_on(revoked.revalidate(
            &grant,
            carosign_core::authorization::AuthorizationBoundary::BeforeDelivery
        )),
        Err(AuthorityRefusal::Revoked)
    );
}

#[test]
fn revalidation_rejects_noncanonical_grant_id_even_with_the_same_uid() {
    use carosign_core::authorization::{AuthorizationBoundary, ValidatedGrant};
    use carosign_core::ids::GrantId;
    let policy = parse(POLICY).unwrap();
    let clock = clock();
    let grants = FilePolicy::with_clock(&policy, authority(), &clock);
    let valid =
        futures_executor::block_on(grants.authorize(&caller(1000, 1), verify_scope(subject(42))))
            .unwrap();
    let mut id = *valid.id().as_bytes();
    id[1] = 9;
    let invalid = ValidatedGrant::issue_for_port(
        GrantId::from_bytes(id).unwrap(),
        valid.issuer(),
        valid.scope().clone(),
        valid.boot(),
        valid.deadline_ns(),
    );
    assert_eq!(
        futures_executor::block_on(
            grants.revalidate(&invalid, AuthorizationBoundary::BeforeDispatch)
        ),
        Err(AuthorityRefusal::Revoked)
    );
}

#[test]
fn grant_deadline_is_relative_to_boot_time_and_checked_at_each_boundary() {
    use carosign_core::authorization::AuthorizationBoundary;
    let policy = parse(POLICY).unwrap();
    let clock = clock();
    clock.advance(5_000_000_000);
    let authority = FilePolicy::with_clock(&policy, authority(), &clock);
    let grant = futures_executor::block_on(
        authority.authorize(&caller(1000, 1), verify_scope(subject(42))),
    )
    .unwrap();
    assert_eq!(grant.deadline_ns(), 35_000_000_000);
    clock.advance(29_999_999_999);
    assert!(
        futures_executor::block_on(
            authority.revalidate(&grant, AuthorizationBoundary::BeforeDispatch)
        )
        .is_ok()
    );
    clock.advance(1);
    assert_eq!(
        futures_executor::block_on(
            authority.revalidate(&grant, AuthorizationBoundary::BeforeDelivery)
        ),
        Err(AuthorityRefusal::Expired)
    );
    let other_boot = StepClock::new(BootId::from_bytes([8; 16]).unwrap());
    let after_reboot = FilePolicy::with_clock(&policy, self::authority(), &other_boot);
    assert_eq!(
        futures_executor::block_on(
            after_reboot.revalidate(&grant, AuthorizationBoundary::BeforeDispatch)
        ),
        Err(AuthorityRefusal::Expired)
    );
}

#[test]
fn missing_clock_zero_lifetime_and_overflow_do_not_issue_grants() {
    let policy = parse(POLICY).unwrap();
    let no_clock = FilePolicy::new(&policy, authority(), BootId::from_bytes([9; 16]).unwrap());
    assert_eq!(
        futures_executor::block_on(no_clock.authorize(&caller(1000, 1), verify_scope(subject(42))))
            .unwrap_err(),
        AuthorityRefusal::Unavailable
    );
    let clock = clock();
    let zero = parse("uid=1000 subject=42 actions=verify lifetime=0").unwrap();
    let zero_policy = FilePolicy::with_clock(&zero, authority(), &clock);
    assert_eq!(
        futures_executor::block_on(
            zero_policy.authorize(&caller(1000, 1), verify_scope(subject(42)))
        )
        .unwrap_err(),
        AuthorityRefusal::Expired
    );
    let huge = parse("uid=1000 subject=42 actions=verify lifetime=18446744073709551615").unwrap();
    let huge_policy = FilePolicy::with_clock(&huge, authority(), &clock);
    assert_eq!(
        futures_executor::block_on(
            huge_policy.authorize(&caller(1000, 1), verify_scope(subject(42)))
        )
        .unwrap_err(),
        AuthorityRefusal::Unavailable
    );
    clock.advance(u64::MAX - 1);
    let ordinary = FilePolicy::with_clock(&policy, authority(), &clock);
    assert_eq!(
        futures_executor::block_on(ordinary.authorize(&caller(1000, 1), verify_scope(subject(42))))
            .unwrap_err(),
        AuthorityRefusal::Unavailable
    );
}

#[test]
fn empty_policy_permits_nothing() {
    let policy = parse("").expect("пустая политика");
    assert!(policy.is_empty());
    assert!(!policy.permits(1000, 42, Action::Verify));
}
