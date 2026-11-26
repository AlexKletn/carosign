use core::fmt;

use carosign_core::ids::{EnrollmentId, MutationId};

use crate::enrollment_codec;
use crate::journal::{
    ArmMutation, ArmedMutationTicket, CommitEnrollment, CommitReconciliation, MutationObservation,
    MutationRepository, PrepareMutation, PreparedMutation, RegistryCommit,
};
use crate::limits;
use crate::model::{MutationClosure, MutationPhase, MutationRecord, StorageError};
use crate::redb::record::{decode_mutation, encode_mutation};
use crate::store::{MetadataStore, Table};

use super::{
    ReadSchema, check_current_scope, check_dispatch_outbox, check_record_lock, read_mutation,
    read_schema, require_legacy_schema,
};

/// The log is on a specific vault.
pub struct StoreJournal<S: MetadataStore> {
    pub(super) store: S,
    pub(super) legacy_fixture: bool,
}

impl<S: MetadataStore> fmt::Debug for StoreJournal<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StoreJournal(..)")
    }
}

impl<S: MetadataStore> StoreJournal<S> {
    /// Log on top of the vault.
    pub fn new(store: S) -> Self {
        Self {
            store,
            legacy_fixture: false,
        }
    }

    /// Creates historical data exclusively in debugging checks.
    #[cfg(all(feature = "legacy-fixtures", debug_assertions))]
    pub fn legacy_fixture(store: S) -> Self {
        Self {
            store,
            legacy_fixture: true,
        }
    }

    /// The storehouse below it (to read the registry).
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Reading the change record.
    pub fn record(&self, mutation: MutationId) -> Result<MutationRecord, StorageError> {
        let tx = self.store.begin_read()?;
        let schema = read_schema(&*tx)?;
        let bytes =
            tx.get(Table::Mutations, mutation.as_bytes())?
                .ok_or(StorageError::Corrupt {
                    what: "запись изменения отсутствует",
                })?;
        let record = read_mutation(schema, mutation.as_bytes(), &bytes)?;
        if record.mutation != mutation {
            return Err(StorageError::Corrupt {
                what: "ключ изменения не совпадает с записью",
            });
        }
        Ok(record)
    }

    /// Blocking the area (if any).
    pub fn scope_lock(
        &self,
        scope: &[u8; limits::SCOPE_LOCK_KEY_LEN],
    ) -> Result<Option<MutationId>, StorageError> {
        let tx = self.store.begin_read()?;
        match tx.get(Table::MutationScopeLock, scope)? {
            Some(bytes) => {
                let id = MutationId::try_from(&bytes[..]).map_err(|_| StorageError::Corrupt {
                    what: "блокировка области",
                })?;
                Ok(Some(id))
            }
            None => Ok(None),
        }
    }

    /// Registration record (to verify that the area is actually being maintained).
    pub fn enrollment(
        &self,
        id: EnrollmentId,
    ) -> Result<Option<crate::enrollment_codec::EnrollmentRecord>, StorageError> {
        let tx = self.store.begin_read()?;
        let schema = read_schema(&*tx)?;
        match tx.get(Table::Enrollments, id.as_bytes())? {
            Some(bytes) => {
                let record = match schema {
                    ReadSchema::V3 => enrollment_codec::decode_v3(&bytes),
                    ReadSchema::Legacy => enrollment_codec::decode(&bytes),
                    ReadSchema::V4 => {
                        let decoded = crate::installation_records::checked_v4_record_as_v3(
                            crate::installation_records::MigratableRecord::Enrollment,
                            id.as_bytes(),
                            &bytes,
                        )
                        .map_err(|_| StorageError::Corrupt {
                            what: "неверная запись регистрации v4",
                        })?;
                        enrollment_codec::decode_v3(&decoded)
                    }
                }
                .map_err(|_| StorageError::Corrupt {
                    what: "запись реестра не разбирается",
                })?;
                if record.enrollment.id != id {
                    return Err(StorageError::Corrupt {
                        what: "ключ регистрации не совпадает с записью",
                    });
                }
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }
}

impl<S: MetadataStore> MutationRepository for StoreJournal<S> {
    fn prepare(&self, input: PrepareMutation) -> Result<PreparedMutation, StorageError> {
        crate::journal::validate_prepare(&input)?;
        let record = MutationRecord {
            mutation: input.mutation,
            operation: input.operation,
            subject: input.subject,
            authenticator: input.authenticator,
            expected_generation: input.expected_generation,
            phase: MutationPhase::Prepared,
            worker: None,
            epoch: None,
            command: None,
            recovery_ref: None,
            provider_observation_digest: None,
            terminal_outcome: None,
            plan: Some(input.plan.clone()),
        };
        record.validate()?;

        let mut tx = self.store.begin_write()?;
        require_legacy_schema(&*tx, self.legacy_fixture)?;

        if tx
            .get(Table::Mutations, record.mutation.as_bytes())?
            .is_some()
        {
            tx.abort();
            return Err(StorageError::GenerationConflict);
        }

        // The operation number belongs to a single mutation, including when they belong to
        // different blocked areas.
        for (key, value) in tx.scan_prefix(Table::Mutations, &[])? {
            let existing = crate::redb::record::decode_mutation(&value)?;
            if key.as_slice() != existing.mutation.as_bytes() {
                return Err(StorageError::Corrupt {
                    what: "ключ изменения не совпадает с записью",
                });
            }
            if existing.operation == record.operation {
                return Err(StorageError::GenerationConflict);
            }
        }

        check_current_scope(&*tx, &record)?;

        // ⚠️ One uncompleted mutation per region (Document 23).
        if tx
            .get(Table::MutationScopeLock, &record.scope_key())?
            .is_some()
        {
            tx.abort();
            return Err(StorageError::GenerationConflict);
        }

        tx.put(
            Table::Mutations,
            record.mutation.as_bytes(),
            &encode_mutation(&record)?,
        )?;
        tx.put(
            Table::MutationScopeLock,
            &record.scope_key(),
            record.mutation.as_bytes(),
        )?;
        let receipt = tx.commit_durable()?;
        Ok(PreparedMutation { record, receipt })
    }

    fn arm(&self, input: ArmMutation) -> Result<ArmedMutationTicket, StorageError> {
        let mut tx = self.store.begin_write()?;
        require_legacy_schema(&*tx, self.legacy_fixture)?;
        let bytes =
            tx.get(Table::Mutations, input.mutation.as_bytes())?
                .ok_or(StorageError::Corrupt {
                    what: "изменение не подготовлено",
                })?;
        let mut record = decode_mutation(&bytes)?;

        if record.mutation != input.mutation {
            tx.abort();
            return Err(StorageError::Corrupt {
                what: "ключ изменения не совпадает с записью",
            });
        }
        if tx
            .get(Table::MutationScopeLock, &record.scope_key())?
            .as_deref()
            != Some(input.mutation.as_bytes().as_slice())
        {
            tx.abort();
            return Err(StorageError::Corrupt {
                what: "блокировка области отсутствует или принадлежит другому изменению",
            });
        }

        // ⚠️ Permission is granted only from `Prepared` and only once.
        if record.phase != MutationPhase::Prepared || record.command.is_some() {
            tx.abort();
            return Err(StorageError::GenerationConflict);
        }

        check_current_scope(&*tx, &record)?;

        record.phase = MutationPhase::DispatchArmed;
        record.worker = Some(input.worker);
        record.epoch = Some(input.epoch);
        record.command = Some(input.command);
        record.recovery_ref = input.recovery_ref.clone();

        tx.put(
            Table::Mutations,
            record.mutation.as_bytes(),
            &encode_mutation(&record)?,
        )?;

        // ⚠️ Entering the queue is in the**same* transaction as the permission.
        let outbox_key = crate::redb::codec::outbox_key(record.operation.as_bytes(), 0, 0);
        if tx.get(Table::EffectOutbox, &outbox_key)?.is_some() {
            tx.abort();
            return Err(StorageError::GenerationConflict);
        }
        tx.put(Table::EffectOutbox, &outbox_key, input.command.as_bytes())?;

        let receipt = tx.commit_durable()?;
        let _ = receipt; // The ticket is issued only after this line.

        Ok(ArmedMutationTicket::new(
            record.mutation,
            record.operation,
            input.command,
            input.worker,
            input.epoch,
            record.scope_key(),
            None,
        ))
    }

    fn record_observation(&self, input: MutationObservation) -> Result<(), StorageError> {
        let mut tx = self.store.begin_write()?;
        require_legacy_schema(&*tx, self.legacy_fixture)?;
        let bytes =
            tx.get(Table::Mutations, input.mutation.as_bytes())?
                .ok_or(StorageError::Corrupt {
                    what: "изменение не подготовлено",
                })?;
        let mut record = decode_mutation(&bytes)?;
        check_record_lock(&*tx, input.mutation, &record)?;
        // Observation of the provider is permissible only after a stable arm;
        // NoChange/Completed should not be taken as an observation.
        if record.phase != MutationPhase::DispatchArmed
            || input.phase != MutationPhase::ProviderObserved
        {
            tx.abort();
            return Err(StorageError::GenerationConflict);
        }
        check_dispatch_outbox(&*tx, &record)?;
        record.phase = input.phase;
        record.recovery_ref = input.recovery_ref.clone();
        tx.put(
            Table::Mutations,
            record.mutation.as_bytes(),
            &encode_mutation(&record)?,
        )?;
        tx.commit_durable()?;
        Ok(())
    }

    fn commit_registry(&self, input: CommitEnrollment) -> Result<RegistryCommit, StorageError> {
        let tx = self.store.begin_write()?;
        require_legacy_schema(&*tx, self.legacy_fixture)?;
        let bytes =
            tx.get(Table::Mutations, input.mutation.as_bytes())?
                .ok_or(StorageError::Corrupt {
                    what: "изменение не подготовлено",
                })?;
        let record = decode_mutation(&bytes)?;
        check_record_lock(&*tx, input.mutation, &record)?;

        // ⚠️ Fixing - only from the observed phase.
        if record.phase != MutationPhase::ProviderObserved {
            tx.abort();
            return Err(StorageError::GenerationConflict);
        }

        // The contract doesn't carry a validated bundle, a CAS result, or the origin of the
        // surveillance, and you can't change the registry without it.
        tx.abort();
        Err(StorageError::Unavailable)
    }

    fn commit_reconciliation(
        &self,
        input: CommitReconciliation,
    ) -> Result<RegistryCommit, StorageError> {
        let tx = self.store.begin_write()?;
        require_legacy_schema(&*tx, self.legacy_fixture)?;
        let bytes =
            tx.get(Table::Mutations, input.mutation.as_bytes())?
                .ok_or(StorageError::Corrupt {
                    what: "изменение не подготовлено",
                })?;
        let record = decode_mutation(&bytes)?;
        check_record_lock(&*tx, input.mutation, &record)?;

        if !matches!(
            record.phase,
            MutationPhase::NeedsReconciliation | MutationPhase::ProviderObserved
        ) {
            tx.abort();
            return Err(StorageError::GenerationConflict);
        }

        // RegistryAction It is not in itself an independent testimony.
        tx.abort();
        Err(StorageError::Unavailable)
    }

    fn finalize(&self, mutation: MutationId, closure: MutationClosure) -> Result<(), StorageError> {
        if !closure.releases_scope() {
            // ⚠️ Verification doesn't close the area.
            return self.mark_uncertain(mutation);
        }
        // The current contract does not transmit quiescence confirmation or store
        // terminal_outcomeYou can't unlock the area without them (B-01).
        Err(StorageError::Corrupt {
            what: "неподтверждённое завершение изменения",
        })
    }

    fn mark_uncertain(&self, mutation: MutationId) -> Result<(), StorageError> {
        let mut tx = self.store.begin_write()?;
        match tx.get(Table::Meta, crate::redb::backend::SCHEMA_KEY)? {
            Some(marker) if marker == crate::mutation_store::SCHEMA_V4 => {
                let bytes = tx.get(Table::Mutations, mutation.as_bytes())?.ok_or(
                    StorageError::Corrupt {
                        what: "изменение не подготовлено",
                    },
                )?;
                let decoded = crate::installation_records::checked_v4_record_as_v3(
                    crate::installation_records::MigratableRecord::Mutation,
                    mutation.as_bytes(),
                    &bytes,
                )
                .map_err(|_| StorageError::Corrupt {
                    what: "неверная запись изменения v4",
                })?;
                let record = crate::mutation_records::decode_mutation(&decoded)?;
                if record.mutation != mutation {
                    return Err(StorageError::Corrupt {
                        what: "ключ изменения не совпадает с записью",
                    });
                }
                crate::mutation_store::stage_needs_reconciliation_v4(
                    &mut *tx,
                    mutation,
                    record.operation,
                )?;
                tx.commit_durable()?;
                return Ok(());
            }
            Some(marker) if marker == crate::redb::backend::SCHEMA_V3 => {
                let bytes = tx.get(Table::Mutations, mutation.as_bytes())?.ok_or(
                    StorageError::Corrupt {
                        what: "изменение не подготовлено",
                    },
                )?;
                let record = crate::mutation_records::decode_mutation(&bytes)?;
                if record.mutation != mutation {
                    return Err(StorageError::Corrupt {
                        what: "ключ изменения не совпадает с записью",
                    });
                }
                crate::mutation_records::stage_needs_reconciliation(
                    &mut *tx,
                    mutation,
                    record.operation,
                )?;
                tx.commit_durable()?;
                return Ok(());
            }
            Some(_) => {
                return Err(StorageError::Corrupt {
                    what: "неизвестный маркер схемы",
                });
            }
            None if self.legacy_fixture => {}
            None => {
                return Err(StorageError::Corrupt {
                    what: "маркер схемы отсутствует",
                });
            }
        }
        let bytes =
            tx.get(Table::Mutations, mutation.as_bytes())?
                .ok_or(StorageError::Corrupt {
                    what: "изменение не подготовлено",
                })?;
        let mut record = decode_mutation(&bytes)?;
        check_record_lock(&*tx, mutation, &record)?;
        // Confirmation of a previous transaction may have been lost after recording.
        // Its actual result needs restoration; it cannot be erased.
        if matches!(
            record.phase,
            MutationPhase::RegistryCommitted | MutationPhase::NoChange | MutationPhase::Completed
        ) {
            tx.abort();
            return Err(StorageError::GenerationConflict);
        }
        if matches!(
            record.phase,
            MutationPhase::DispatchArmed | MutationPhase::ProviderObserved
        ) {
            check_dispatch_outbox(&*tx, &record)?;
        }
        record.phase = MutationPhase::NeedsReconciliation;
        tx.put(
            Table::Mutations,
            mutation.as_bytes(),
            &encode_mutation(&record)?,
        )?;
        // Blockage stays.
        tx.commit_durable()?;
        Ok(())
    }

    fn scan_incomplete(&self) -> Result<Vec<MutationRecord>, StorageError> {
        let tx = self.store.begin_read()?;
        let schema = read_schema(&*tx)?;
        if matches!(schema, ReadSchema::V4) {
            // Verify the log with locks, queue and idempotence in the same picture before
            // issuing the list for recovery.
            crate::mutation_store::scan_prepared_v4_snapshot(&*tx)?;
        }
        let mut out = Vec::new();
        for (key, value) in tx.scan_prefix(Table::Mutations, &[])? {
            let record = read_mutation(schema, &key, &value)?;
            if key.as_slice() != record.mutation.as_bytes() {
                return Err(StorageError::Corrupt {
                    what: "ключ изменения не совпадает с записью",
                });
            }
            if if !matches!(schema, ReadSchema::Legacy) {
                record.phase != MutationPhase::Completed
            } else {
                crate::recovery::decide_for(&record) != crate::recovery::RecoveryDecision::Closed
            } {
                out.push(record);
            }
        }
        out.sort_by_key(|record| record.mutation);
        Ok(out)
    }
}
