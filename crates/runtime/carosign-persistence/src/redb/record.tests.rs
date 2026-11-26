use super::*;
use crate::model::MutationPhase;

fn record() -> MutationRecord {
    MutationRecord {
        mutation: MutationId::from_bytes([1u8; 16]).expect("валид"),
        operation: OperationId::from_bytes([2u8; 16]).expect("валид"),
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([3u8; 16]).expect("валид"),
            subject: SubjectId::from_bytes([4u8; 16]).expect("валид"),
        },
        authenticator: AuthenticatorId::from_bytes([5u8; 16]).expect("валид"),
        expected_generation: Some(EnrollmentGeneration::FIRST),
        phase: MutationPhase::DispatchArmed,
        worker: Some(WorkerInstanceId::from_bytes([6u8; 16]).expect("валид")),
        epoch: Some(DeviceEpoch::FIRST),
        command: Some(CommandId::from_bytes([7u8; 16]).expect("валид")),
        recovery_ref: Some(OpaqueProviderRef::new(vec![9, 9, 9]).expect("годится")),
        provider_observation_digest: None,
        terminal_outcome: None,
        plan: Some(
            crate::mutation_plan::MutationPlan::build(
                crate::mutation_plan::Transition::EnrollNew,
                None,
                Some(carosign_core::ids::EnrollmentId::from_bytes([8u8; 16]).expect("валид")),
                1,
                Vec::new(),
                crate::mutation_plan::Scope {
                    subject: SubjectRef {
                        authority: AuthorityId::from_bytes([3u8; 16]).expect("валид"),
                        subject: SubjectId::from_bytes([4u8; 16]).expect("валид"),
                    },
                    authenticator: AuthenticatorId::from_bytes([5u8; 16]).expect("валид"),
                },
            )
            .expect("план"),
        ),
    }
}

#[test]
fn record_round_trip() {
    let original = record();
    let bytes = encode_mutation(&original).expect("кодируется");
    assert_eq!(decode_mutation(&bytes), Ok(original));
}

#[test]
fn persisted_or_incomplete_armed_record_does_not_authorize_dispatch() {
    let mut armed = record();
    assert!(!armed.allows_dispatch());
    armed.command = None;
    assert!(!armed.allows_dispatch());
}

#[test]
fn prepared_record_has_no_worker_or_command() {
    let mut original = record();
    original.phase = MutationPhase::Prepared;
    original.worker = None;
    original.command = None;
    original.epoch = None;
    original.recovery_ref = None;
    let bytes = encode_mutation(&original).expect("кодируется");
    let back = decode_mutation(&bytes).expect("разбирается");
    assert_eq!(back, original);
    assert!(!back.allows_dispatch());
}

#[test]
fn decoded_old_target_generation_must_match_record() {
    use crate::mutation_plan::{
        BindingSnapshot, MutationPlan, Scope, StoredBindingKind, Transition,
    };
    use carosign_core::enrollment::EnrollmentRef;
    use carosign_core::ids::EnrollmentId;

    let mut original = record();
    original.phase = MutationPhase::Prepared;
    original.worker = None;
    original.epoch = None;
    original.command = None;
    let target = EnrollmentRef {
        id: EnrollmentId::from_bytes([8; 16]).unwrap(),
        authenticator: original.authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let snapshot = BindingSnapshot {
        enrollment: target,
        subject: original.subject,
        kind: StoredBindingKind::ProviderOpaque,
        provider_ref_digest: [5; 32],
        format_id: "test".into(),
        format_version: 1,
    };
    original.plan = Some(
        MutationPlan::build(
            Transition::Remove,
            Some(target),
            None,
            0,
            vec![snapshot],
            Scope {
                subject: original.subject,
                authenticator: original.authenticator,
            },
        )
        .unwrap(),
    );
    original.expected_generation = Some(target.generation);
    let valid = encode_mutation(&original).unwrap();
    assert_eq!(decode_mutation(&valid), Ok(original.clone()));

    for generation in [None, Some(target.generation.next().unwrap())] {
        let mut inconsistent = original.clone();
        inconsistent.expected_generation = generation;
        assert!(matches!(
            encode_mutation(&inconsistent),
            Err(StorageError::Corrupt { .. })
        ));
        let mut envelope = Envelope::decode(&valid).unwrap();
        envelope.payload[81] = u8::from(generation.is_some());
        envelope.payload[82..90]
            .copy_from_slice(&generation.map_or(0, |value| value.value()).to_be_bytes());
        assert!(matches!(
            decode_mutation(&envelope.encode().unwrap()),
            Err(StorageError::Corrupt { .. })
        ));
        assert_eq!(
            crate::recovery::decide_for(&inconsistent),
            crate::recovery::RecoveryDecision::NeedsDecision
        );
    }
}

#[test]
fn absent_fields_cannot_hide_nonzero_bytes() {
    let mut original = record();
    original.phase = MutationPhase::Prepared;
    original.expected_generation = None;
    original.worker = None;
    original.epoch = None;
    original.command = None;
    original.recovery_ref = None;
    let bytes = encode_mutation(&original).unwrap();
    assert_eq!(decode_mutation(&bytes).unwrap(), original);
    // Five IDs, phase and fixed fields with separate features.
    for offset in [82, 91, 108, 117, 137] {
        let mut envelope = Envelope::decode(&bytes).unwrap();
        envelope.payload[offset] = 1;
        assert!(matches!(
            decode_mutation(&envelope.encode().unwrap()),
            Err(StorageError::Corrupt { .. })
        ));
    }
}

#[test]
fn corrupted_record_is_rejected() {
    let bytes = encode_mutation(&record()).expect("кодируется");
    let mut damaged = bytes.clone();
    damaged[12] ^= 0xff;
    assert!(matches!(
        decode_mutation(&damaged),
        Err(StorageError::Corrupt { .. })
    ));
}

#[test]
fn well_checksummed_record_with_wrong_plan_or_missing_fence_is_rejected() {
    let mut invalid = record();
    invalid.plan.as_mut().unwrap().scope_digest[0] ^= 1;
    assert!(encode_mutation(&invalid).is_err());
    let mut envelope = Envelope::decode(&encode_mutation(&record()).unwrap()).unwrap();
    *envelope.payload.last_mut().unwrap() ^= 1;
    assert!(matches!(
        decode_mutation(&envelope.encode().unwrap()),
        Err(StorageError::Corrupt { .. })
    ));
    let mut invalid = record();
    invalid.command = None;
    assert!(matches!(
        decode_mutation(&encode_mutation(&invalid).unwrap()),
        Err(StorageError::Corrupt { .. })
    ));
    let mut prepared = record();
    prepared.phase = MutationPhase::Prepared;
    assert_eq!(
        decode_mutation(&encode_mutation(&prepared).unwrap()).unwrap(),
        prepared
    );
}

#[test]
fn unknown_presence_marker_is_not_treated_as_absent() {
    let bytes = encode_mutation(&record()).unwrap();
    let mut envelope = Envelope::decode(&bytes).unwrap();
    // Five 16-byte IDs, then phase and generation sign.
    envelope.payload[81] = 2;
    let encoded = envelope.encode().unwrap();
    assert!(matches!(
        decode_mutation(&encoded),
        Err(StorageError::Corrupt { .. })
    ));
}
