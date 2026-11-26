use super::{
    BlockedMutationScope, MetadataStore, MutationId, MutationPhase, RecoveryDecision,
    StartupScopeScan, StorageError, check_registry_v3, decide_for, fmt,
};

/// Status description for readiness of service.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct StartupScan {
    /// Records that didn't touch anything anymore.
    pub safe_to_close: Vec<MutationId>,
    /// Records in which the hardware may have changed.
    pub needing_reconciliation: Vec<MutationId>,
    /// Records that the world cannot judge.
    pub needs_decision: Vec<MutationId>,
    /// Records previously closed.
    pub closed: Vec<MutationId>,
}

impl StartupScan {
    /// Verifies the v3 magazine image. Version selection and migration remain at launch.
    ///
    /// Having a queue record doesn't allow re-sending. Unknown queue connections are
    /// rejected: the circuit doesn't yet store the kind of effect.
    pub fn scan_v3(store: &dyn MetadataStore) -> Result<Self, StorageError> {
        Ok(Self::scan_v3_with_scopes(store)?.scan)
    }

    /// Checks the journal and gets the areas from the same photo.
    /// Selecting the version of the scheme and stopping the admission to the call is the
    /// responsibility of the start.
    pub fn scan_v3_with_scopes(
        store: &dyn MetadataStore,
    ) -> Result<StartupScopeScan, StorageError> {
        let tx = store.begin_read()?;
        Self::scan_v3_snapshot(&*tx)
    }

    /// Verifies the transmitted image without opening the second transaction.
    pub(crate) fn scan_v3_snapshot(
        tx: &dyn crate::store::ReadTransaction,
    ) -> Result<StartupScopeScan, StorageError> {
        use crate::store::Table;
        use std::collections::{BTreeMap, BTreeSet};
        check_registry_v3(tx)?;
        // Repetitions of requests after restarting are based on these records; their damage
        // cannot be considered an empty log or a new request.
        let mut idempotency = Vec::new();
        for (key, value) in tx.scan_prefix(Table::Idempotency, &[])? {
            idempotency.push(crate::mutation_records::decode_idempotency(&key, &value)?);
        }
        let mut scan = Self::default();
        let mut blocked = Vec::new();
        let mut records = BTreeMap::new();
        let mut operations = BTreeSet::new();
        let mut active_scopes = BTreeSet::new();
        for (key, value) in tx.scan_prefix(Table::Mutations, &[])? {
            let record = crate::mutation_records::decode_mutation(&value)?;
            if key.as_slice() != record.mutation.as_bytes() || !operations.insert(record.operation)
            {
                return Err(StorageError::Corrupt {
                    what: "неверный ключ изменения или повтор операции",
                });
            }
            if record.phase != MutationPhase::Completed
                && tx
                    .get(Table::MutationScopeLock, &record.scope_key())?
                    .as_deref()
                    != Some(record.mutation.as_bytes().as_slice())
            {
                return Err(StorageError::Corrupt {
                    what: "нет соответствующей блокировки области",
                });
            }
            if record.phase != MutationPhase::Completed {
                if !active_scopes.insert(record.scope_key()) {
                    return Err(StorageError::Corrupt {
                        what: "пересекающиеся области изменений",
                    });
                }
                blocked.push(BlockedMutationScope {
                    mutation: record.mutation,
                    subject: record.subject,
                    authenticator: record.authenticator,
                });
            }
            match record.phase {
                MutationPhase::Prepared => scan.safe_to_close.push(record.mutation),
                MutationPhase::DispatchArmed
                | MutationPhase::ProviderObserved
                | MutationPhase::RegistryCommitted
                | MutationPhase::NeedsReconciliation => {
                    scan.needing_reconciliation.push(record.mutation)
                }
                // NoChange It is still awaiting confirmation of cleaning and completion.
                MutationPhase::NoChange => scan.needs_decision.push(record.mutation),
                MutationPhase::Completed => scan.closed.push(record.mutation),
            }
            records.insert(record.mutation, record);
        }
        let mut idempotent_mutations = BTreeSet::new();
        for entry in idempotency {
            if let Some(mutation) = entry.mutation {
                if records
                    .get(&mutation)
                    .is_none_or(|record| record.operation != entry.operation)
                {
                    return Err(StorageError::Corrupt {
                        what: "идемпотентная запись не соответствует журналу изменения",
                    });
                }
                idempotent_mutations.insert(mutation);
            } else if operations.contains(&entry.operation) {
                return Err(StorageError::Corrupt {
                    what: "операция проверки совпадает с изменением в журнале",
                });
            }
        }
        if records.values().any(|record| {
            record.phase != MutationPhase::Completed
                && !idempotent_mutations.contains(&record.mutation)
        }) {
            return Err(StorageError::Corrupt {
                what: "нет идемпотентной записи незавершённого изменения",
            });
        }
        for (scope, value) in tx.scan_prefix(Table::MutationScopeLock, &[])? {
            let mutation =
                MutationId::try_from(value.as_slice()).map_err(|_| StorageError::Corrupt {
                    what: "неверная блокировка области",
                })?;
            if records.get(&mutation).is_none_or(|record| {
                record.phase == MutationPhase::Completed
                    || record.scope_key().as_slice() != scope.as_slice()
            }) {
                return Err(StorageError::Corrupt {
                    what: "блокировка не соответствует журналу",
                });
            }
        }
        let mut linked = BTreeSet::new();
        let mut commands = BTreeSet::new();
        let mut other_commands = BTreeSet::new();
        for (key, value) in tx.scan_prefix(Table::EffectOutbox, &[])? {
            let outbox = crate::mutation_records::decode_outbox(&key, &value)?;
            if let Some(mutation) = outbox.mutation {
                let record = records.get(&mutation).ok_or(StorageError::Corrupt {
                    what: "очередь ссылается на отсутствующее изменение",
                })?;
                crate::mutation_records::validate_mutation_outbox(record, &outbox)?;
                let command = outbox.command.ok_or(StorageError::Corrupt {
                    what: "нет команды изменения в очереди",
                })?;
                if !linked.insert(mutation)
                    || other_commands.contains(&command)
                    || !commands.insert(command)
                {
                    return Err(StorageError::Corrupt {
                        what: "повтор команды или изменения в очереди",
                    });
                }
            } else if let Some(command) = outbox.command {
                if commands.contains(&command) {
                    return Err(StorageError::Corrupt {
                        what: "команда изменения занята другим эффектом",
                    });
                }
                other_commands.insert(command);
            }
        }
        for record in records.values() {
            if (matches!(
                record.phase,
                MutationPhase::DispatchArmed
                    | MutationPhase::ProviderObserved
                    | MutationPhase::RegistryCommitted
                    | MutationPhase::NeedsReconciliation
            ) || record.phase == MutationPhase::NoChange && record.command.is_some())
                && !linked.contains(&record.mutation)
            {
                return Err(StorageError::Corrupt {
                    what: "нет очереди отправленного изменения",
                });
            }
        }
        scan.safe_to_close.sort();
        scan.needing_reconciliation.sort();
        scan.needs_decision.sort();
        scan.closed.sort();
        blocked.sort_by_key(|entry| entry.mutation);
        Ok(StartupScopeScan { scan, blocked })
    }

    /// Takes all the notes down in one reading.
    pub fn scan(store: &dyn MetadataStore) -> Result<Self, StorageError> {
        let tx = store.begin_read()?;
        let mut scan = Self::default();
        let mut records = std::collections::BTreeMap::new();
        let mut operations = std::collections::BTreeSet::new();
        for (key, value) in tx.scan_prefix(crate::store::Table::Mutations, &[])? {
            let record = crate::redb::record::decode_mutation(&value)?;
            if key.as_slice() != record.mutation.as_bytes() || !operations.insert(record.operation)
            {
                return Err(StorageError::Corrupt {
                    what: "неверный ключ изменения или повтор операции",
                });
            }
            let decision = decide_for(&record);
            // Completed old recording without terminal_outcome It is not proven closed and
            // also requires a valid area lock.
            if decision != RecoveryDecision::Closed
                && tx
                    .get(crate::store::Table::MutationScopeLock, &record.scope_key())?
                    .as_deref()
                    != Some(record.mutation.as_bytes().as_slice())
            {
                return Err(StorageError::Corrupt {
                    what: "нет соответствующей блокировки области",
                });
            }
            match decision {
                RecoveryDecision::CloseAsNoChange => scan.safe_to_close.push(record.mutation),
                RecoveryDecision::BlockAndReconcile => {
                    scan.needing_reconciliation.push(record.mutation)
                }
                RecoveryDecision::NeedsDecision => scan.needs_decision.push(record.mutation),
                RecoveryDecision::Closed => scan.closed.push(record.mutation),
            }
            records.insert(record.mutation, record);
        }
        // We also check for feedback: a lost log or locking someone else’s area should not
        // remain invisible to run.
        for (scope, value) in tx.scan_prefix(crate::store::Table::MutationScopeLock, &[])? {
            let mutation =
                MutationId::try_from(value.as_slice()).map_err(|_| StorageError::Corrupt {
                    what: "неверная блокировка области",
                })?;
            if records
                .get(&mutation)
                .map(|record| record.scope_key().to_vec())
                != Some(scope)
            {
                return Err(StorageError::Corrupt {
                    what: "блокировка не соответствует журналу",
                });
            }
        }
        // The temporary format of the queue stores only CommandIdVerify both sides of the
        // communication in one shot; this does not replace the regulatory outbox.
        let mut expected_outbox = std::collections::BTreeMap::new();
        for record in records.values() {
            if let (phase, Some(command)) = (record.phase, record.command) {
                if phase == MutationPhase::Prepared {
                    continue;
                }
                let required = matches!(
                    record.phase,
                    MutationPhase::DispatchArmed
                        | MutationPhase::ProviderObserved
                        | MutationPhase::RegistryCommitted
                        | MutationPhase::NeedsReconciliation
                );
                let key = crate::redb::codec::outbox_key(record.operation.as_bytes(), 0, 0);
                if expected_outbox
                    .insert(key.to_vec(), (command.as_bytes().to_vec(), required))
                    .is_some()
                {
                    return Err(StorageError::Corrupt {
                        what: "две записи используют одну очередь",
                    });
                }
            }
        }
        for (key, value) in tx.scan_prefix(crate::store::Table::EffectOutbox, &[])? {
            if expected_outbox.remove(&key).map(|(command, _)| command) != Some(value) {
                return Err(StorageError::Corrupt {
                    what: "очередь не соответствует журналу",
                });
            }
        }
        if expected_outbox.values().any(|(_, required)| *required) {
            return Err(StorageError::Corrupt {
                what: "нет очереди отправленного изменения",
            });
        }
        scan.safe_to_close.sort();
        scan.closed.sort();
        scan.needing_reconciliation.sort();
        scan.needs_decision.sort();
        Ok(scan)
    }

    /// Is it possible to declare the service ready?
    ///
    /// ⚠️ Readiness **not ** requires an empty list: the service can be `Degraded` But if
    /// there's nothing to judge by, there's no readyness.
    pub fn allows_ready(&self) -> bool {
        self.needs_decision.is_empty()
    }
}

impl fmt::Display for StartupScan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "закрыть: {}, сверять: {}, решить: {}",
            self.safe_to_close.len(),
            self.needing_reconciliation.len(),
            self.needs_decision.len()
        )
    }
}
