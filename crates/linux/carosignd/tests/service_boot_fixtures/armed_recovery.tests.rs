use super::*;
use carosign_core::{
    device::DeviceEpoch,
    enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef},
    identity::SubjectRef,
    ids::{
        AuthenticatorId, AuthorityId, CommandId, EnrollmentId, IdempotencyKey, MutationId,
        OperationId, SubjectId, WorkerInstanceId,
    },
};
use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
use carosign_persistence::{
    enrollment_codec::{EnrollmentRecord, EnrollmentStatus},
    journal::{ArmMutation, PrepareMutation},
    model::{IdempotencyRecord, MutationPhase, MutationRecord, MutationTerminalOutcome},
    mutation_plan::{MutationPlan, Scope, StoredBindingKind, Transition},
    mutation_store::{
        RecoveryEvidence, read_completed_v4, read_mutation_by_operation_v4, stage_arm_v4,
        stage_prepared_v4,
    },
    recovery::MutationSnapshot,
    redb::backend::RedbStore,
    store::{MetadataStore, Table},
};
use carosignd::bootstrap::{ArmedRecoverySource, recover_guarded_with_evidence};

struct Source {
    observation: Option<Vec<EnrollmentRecord>>,
    cleanup: bool,
    foreign_epoch: bool,
}

impl ArmedRecoverySource for Source {
    fn verify(&self, snapshot: &MutationSnapshot) -> Option<RecoveryEvidence> {
        let record = &snapshot.mutation;
        let epoch = record.epoch?;
        Some(RecoveryEvidence {
            mutation: record.mutation,
            operation: record.operation,
            command: record.command?,
            worker: record.worker?,
            epoch: if self.foreign_epoch {
                epoch.next().unwrap()
            } else {
                epoch
            },
            scope: Scope {
                subject: record.subject,
                authenticator: record.authenticator,
            },
            digest: [52; 32],
            enrollments: self.observation.clone()?,
        })
    }

    fn confirm_cleanup(&self, record: &MutationRecord) -> bool {
        assert!(matches!(
            record.phase,
            MutationPhase::RegistryCommitted | MutationPhase::NoChange
        ));
        self.cleanup
    }
}

#[test]
fn guarded_boot_reconciles_armed_mutation_and_finishes_only_after_cleanup() {
    for committed in [false, true] {
        let path = temp_path(if committed {
            "armed-desired"
        } else {
            "armed-no-change"
        });
        let scope = Scope {
            subject: SubjectRef {
                authority: AuthorityId::from_bytes([1; 16]).unwrap(),
                subject: SubjectId::from_bytes([2; 16]).unwrap(),
            },
            authenticator: AuthenticatorId::from_bytes([3; 16]).unwrap(),
        };
        let mutation = MutationId::from_bytes([4; 16]).unwrap();
        let operation = OperationId::from_bytes([5; 16]).unwrap();
        let enrollment = EnrollmentId::from_bytes([6; 16]).unwrap();
        let prepared = PrepareMutation {
            mutation,
            operation,
            subject: scope.subject,
            authenticator: scope.authenticator,
            expected_generation: None,
            enroll: true,
            plan: MutationPlan::build(
                Transition::EnrollNew,
                None,
                Some(enrollment),
                1,
                vec![],
                scope,
            )
            .unwrap()
            .with_requested_label(scope, "test")
            .unwrap(),
        };
        let request = IdempotencyRecord {
            caller_digest: [7; 32],
            key: IdempotencyKey::from_bytes([8; 16]).unwrap(),
            request_digest: carosign_persistence::mutation_plan::canonical_request_digest(
                scope,
                Transition::EnrollNew,
                None,
                Some("test"),
            )
            .unwrap(),
            operation,
            mutation: Some(mutation),
        };
        let arm = ArmMutation {
            mutation,
            worker: WorkerInstanceId::from_bytes([10; 16]).unwrap(),
            epoch: DeviceEpoch::FIRST,
            command: CommandId::from_bytes([11; 16]).unwrap(),
            recovery_ref: None,
        };
        let store = RedbStore::create_current(&path).unwrap();
        let mut tx = store.begin_write().unwrap();
        stage_prepared_v4(&mut *tx, &prepared, &request).unwrap();
        stage_arm_v4(
            &mut *tx,
            &arm,
            PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 0),
        )
        .unwrap();
        tx.commit_durable().unwrap();
        drop(store);

        let observed = EnrollmentRecord {
            subject: scope.subject,
            enrollment: EnrollmentRef {
                id: enrollment,
                authenticator: scope.authenticator,
                generation: EnrollmentGeneration::FIRST,
            },
            display_label: None,
            label: "test".into(),
            status: EnrollmentStatus::Active,
            provider_ref: OpaqueProviderRef::new(vec![12]).unwrap(),
            format_id: "mock".into(),
            format_version: 1,
            binding_kind: StoredBindingKind::ProviderOpaque,
        };
        let mut source = Source {
            observation: None,
            cleanup: true,
            foreign_epoch: false,
        };
        let (store, recovered, _) =
            recover_guarded_with_evidence(path.to_str().unwrap(), &source).unwrap();
        assert_eq!(recovered.blocked, vec![mutation]);
        assert_eq!(
            read_mutation_by_operation_v4(&store, operation)
                .unwrap()
                .unwrap()
                .phase,
            MutationPhase::DispatchArmed
        );
        drop(store);

        let foreign = Source {
            observation: Some(vec![observed.clone()]),
            foreign_epoch: true,
            ..source
        };
        let (store, recovered, _) =
            recover_guarded_with_evidence(path.to_str().unwrap(), &foreign).unwrap();
        assert_eq!(recovered.blocked, vec![mutation]);
        assert_eq!(
            read_mutation_by_operation_v4(&store, operation)
                .unwrap()
                .unwrap()
                .phase,
            MutationPhase::DispatchArmed
        );
        drop(store);

        if !committed {
            let store = RedbStore::open_current_guarded(&path).unwrap();
            let mut tx = store.begin_write().unwrap();
            carosign_persistence::mutation_store::stage_needs_reconciliation_v4(
                &mut *tx, mutation, operation,
            )
            .unwrap();
            tx.commit_durable().unwrap();
        }

        source = Source {
            observation: Some(if committed { vec![observed] } else { vec![] }),
            cleanup: false,
            foreign_epoch: false,
        };
        let (store, recovered, _) =
            recover_guarded_with_evidence(path.to_str().unwrap(), &source).unwrap();
        assert_eq!(recovered.blocked, vec![mutation]);
        let expected = if committed {
            MutationPhase::RegistryCommitted
        } else {
            MutationPhase::NoChange
        };
        assert_eq!(
            read_mutation_by_operation_v4(&store, operation)
                .unwrap()
                .unwrap()
                .phase,
            expected
        );
        assert!(read_completed_v4(&store, mutation, operation).is_err());
        drop(store);

        if committed {
            let contradictory = Source {
                observation: Some(vec![]),
                cleanup: true,
                foreign_epoch: false,
            };
            let (store, recovered, _) =
                recover_guarded_with_evidence(path.to_str().unwrap(), &contradictory).unwrap();
            assert_eq!(recovered.blocked, vec![mutation]);
            assert!(read_completed_v4(&store, mutation, operation).is_err());
            drop(store);
        }

        source.cleanup = true;
        let (store, recovered, _) =
            recover_guarded_with_evidence(path.to_str().unwrap(), &source).unwrap();
        assert!(recovered.blocked.is_empty());
        let expected = if committed {
            MutationTerminalOutcome::Committed
        } else {
            MutationTerminalOutcome::NoChange
        };
        assert_eq!(
            read_completed_v4(&store, mutation, operation)
                .unwrap()
                .terminal_outcome,
            Some(expected)
        );
        {
            let read = store.begin_read().unwrap();
            assert_eq!(
                read.scan_prefix(Table::Enrollments, &[]).unwrap().len(),
                usize::from(committed)
            );
            assert_eq!(read.scan_prefix(Table::EffectOutbox, &[]).unwrap().len(), 1);
        }
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
