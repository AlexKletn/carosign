//! The path “registrar → set of authorized registrations” (Document 29, D-03).
//!
//! ⚠️ Selection is checked: empty - refusal, someone else - damage to the registry, turned
//! off - does not enter, storage - without other people's bytes.

use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, AuthorityId, EnrollmentId, SubjectId};
use carosign_runtime::authority::{Candidate, Custody, SelectionError, Status, select};

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn authenticator() -> AuthenticatorId {
    AuthenticatorId::from_bytes([3u8; 16]).expect("валид")
}

fn candidate(byte: u8, active: bool, custody: Custody) -> Candidate {
    let status = if active {
        Status::Active
    } else {
        Status::Unavailable
    };
    Candidate {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
            authenticator: authenticator(),
            generation: EnrollmentGeneration::FIRST,
        },
        status,
        custody,
        provider_ref: match custody {
            Custody::Provider => vec![byte; 4],
            Custody::PlatformVault => Vec::new(),
        },
        format_id: Some("goodix-asp".into()),
        format_version: 1,
    }
}

#[test]
fn empty_is_not_enrolled_never_scan_all() {
    assert_eq!(
        select(subject(), authenticator(), DeviceEpoch::FIRST, &[]),
        Err(SelectionError::NotEnrolled)
    );
    let all_off = vec![
        candidate(1, false, Custody::Provider),
        candidate(2, false, Custody::PlatformVault),
    ];
    assert_eq!(
        select(subject(), authenticator(), DeviceEpoch::FIRST, &all_off),
        Err(SelectionError::NotEnrolled)
    );
}

#[test]
fn only_active_enters_the_set() {
    let set = select(
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
        &[
            candidate(1, true, Custody::Provider),
            candidate(2, false, Custody::Provider),
            candidate(3, true, Custody::Provider),
        ],
    )
    .expect("набор");
    assert_eq!(set.len(), 2);
}

#[test]
fn foreign_records_are_corruption() {
    let mut foreign = candidate(4, true, Custody::Provider);
    foreign.enrollment.authenticator = AuthenticatorId::from_bytes([9u8; 16]).expect("валид");
    assert_eq!(
        select(subject(), authenticator(), DeviceEpoch::FIRST, &[foreign]),
        Err(SelectionError::WrongAuthenticator)
    );
}

#[test]
fn pending_reconciliation_refuses_the_whole_verification() {
    // ⚠️ Document 10: You can’t check the registration waiting to be verified, and “missing
    // it” is not the same as “checking others.”
    let mut pending = candidate(9, true, Custody::Provider);
    pending.status = Status::PendingReconciliation;
    assert_eq!(
        select(
            subject(),
            authenticator(),
            DeviceEpoch::FIRST,
            &[candidate(1, true, Custody::Provider), pending]
        ),
        Err(SelectionError::PendingReconciliation)
    );
}

#[test]
fn vault_binding_has_no_foreign_bytes() {
    let set = select(
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
        &[
            candidate(5, true, Custody::Provider),
            candidate(6, true, Custody::PlatformVault),
        ],
    )
    .expect("набор");
    // Opaque - with bytes of the provider; storage - without.
    assert_eq!(set.bindings()[0].provider_ref, vec![5u8; 4]);
    assert!(set.bindings()[1].provider_ref.is_empty());
}

#[test]
fn custody_is_carried_into_the_kind() {
    let set = select(
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
        &[
            candidate(7, true, Custody::Provider),
            candidate(8, true, Custody::PlatformVault),
        ],
    )
    .expect("набор");
    match (set.bindings()[0].kind, set.bindings()[1].kind) {
        (
            carosign_runtime::BindingKind::ProviderOpaque,
            carosign_runtime::BindingKind::ScopedVault,
        ) => {}
        other => panic!("виды связок перепутаны: {other:?}"),
    }
}

#[test]
fn record_becomes_a_candidate_with_its_own_status() {
    // ⚠️ The only way to “record storage → candidate”: status from the record, type of
    // storage – from the explicit field of the record.
    use carosign_persistence::enrollment_codec::{
        EnrollmentRecord as StoredRecord, EnrollmentStatus as StoredStatus,
    };

    let stored = StoredRecord {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([11u8; 16]).expect("валид"),
            authenticator: authenticator(),
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "палец".into(),
        status: StoredStatus::PendingReconciliation,
        provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(vec![1, 2])
            .expect("годится"),
        format_id: "goodix-asp".into(),
        format_version: 1,
        binding_kind: carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque,
    };
    let candidate = Candidate::from_record(&stored);
    assert_eq!(candidate.status, Status::PendingReconciliation);
    assert_eq!(candidate.custody, Custody::Provider);

    // ⚠️ The repository is defined by a **field**, not an empty reference.
    let vault = StoredRecord {
        provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(Vec::new())
            .expect("годится"),
        status: StoredStatus::Active,
        binding_kind: carosign_persistence::mutation_plan::StoredBindingKind::ScopedVault,
        ..stored.clone()
    };
    assert_eq!(
        Candidate::from_record(&vault).custody,
        Custody::PlatformVault
    );
    // ⚠️ Reverse: Empty reference in case of `ProviderOpaque` - Still a provider.
    let provider_empty = StoredRecord {
        provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(Vec::new())
            .expect("годится"),
        ..stored
    };
    assert_eq!(
        Candidate::from_record(&provider_empty).custody,
        Custody::Provider
    );
}
