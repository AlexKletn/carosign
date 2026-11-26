use super::{
    EnrollmentId, EnrollmentRef, Lookup, MetadataStore, StorageError, Table, decode_record, fmt,
};

/// Reading the registry.
pub struct RegistryReader<'a, S: MetadataStore> {
    pub(super) store: &'a S,
}

impl<'a, S: MetadataStore> fmt::Debug for RegistryReader<'a, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RegistryReader(..)")
    }
}

impl<'a, S: MetadataStore> RegistryReader<'a, S> {
    /// Reads the registry on top of the vault.
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }

    /// Finds the registration record.
    ///
    /// ⚠️ Discussion. `enrollment_codec`: shell, schema version and all fields.
    /// “Not cleared up” becomes a mistake, not a “no record.”
    pub fn by_id(&self, id: EnrollmentId) -> Result<Lookup, StorageError> {
        let tx = self.store.begin_read()?;
        match tx.get(Table::Enrollments, id.as_bytes())? {
            None => Ok(Lookup::Missing),
            Some(bytes) => match decode_record(&*tx, id.as_bytes(), &bytes) {
                Ok(record) if record.enrollment.id == id => Ok(Lookup::Found(record)),
                Ok(_) => Err(StorageError::Corrupt {
                    what: "ключ регистрации не совпадает с записью",
                }),
                Err(error) => Err(error),
            },
        }
    }

    /// Checks the client’s expectations by generation.
    pub fn generation_of(&self, reference: EnrollmentRef) -> Result<Option<u64>, StorageError> {
        match self.by_id(reference.id)? {
            Lookup::Missing => Ok(None),
            Lookup::Found(record) if record.enrollment.authenticator == reference.authenticator => {
                Ok(Some(record.enrollment.generation.value()))
            }
            Lookup::Found(_) => Err(StorageError::GenerationConflict),
        }
    }

    /// List of registrations ** pairs ** "subject + authenticator".
    ///
    /// ⚠️ Rule of document 29 (D-03): the check is addressed to this pair.
    /// Scanning one subject or one authenticator would give you a “scan all”.
    ///
    /// ⚠️ Scanned **only its prefix** index: 48 bytes `AuthorityId || SubjectId ||
    /// AuthenticatorId`.
    pub fn by_scope(
        &self,
        scope: &[u8; crate::limits::SCOPE_LOCK_KEY_LEN],
    ) -> Result<Vec<EnrollmentId>, StorageError> {
        let tx = self.store.begin_read()?;
        let mut out = Vec::new();
        for (key, value) in tx.scan_prefix(Table::EnrollmentIndex, scope)? {
            if key.len() != crate::limits::ENROLLMENT_INDEX_KEY_LEN
                || value.as_slice() != &key[crate::limits::SCOPE_LOCK_KEY_LEN..]
            {
                return Err(StorageError::Corrupt {
                    what: "ключ и значение индекса не совпадают",
                });
            }
            let id = EnrollmentId::try_from(&value[..]).map_err(|_| StorageError::Corrupt {
                what: "индекс реестра",
            })?;
            let bytes =
                tx.get(Table::Enrollments, id.as_bytes())?
                    .ok_or(StorageError::Corrupt {
                        what: "индекс ссылается на отсутствующую регистрацию",
                    })?;
            let record = decode_record(&*tx, id.as_bytes(), &bytes)?;
            if record.enrollment.id != id
                || crate::redb::codec::enrollment_index_key(&record).as_slice() != key.as_slice()
            {
                return Err(StorageError::Corrupt {
                    what: "индекс не соответствует регистрации",
                });
            }
            out.push(id);
        }
        out.sort();
        Ok(out)
    }

    /// A trusted full picture of all the entities in a single transaction.
    /// It does not allow fingerprint search and does not replace mutation area blocking.
    pub fn by_authenticator(
        &self,
        authenticator: carosign_core::ids::AuthenticatorId,
    ) -> Result<Vec<crate::projection::RegistryEntry>, StorageError> {
        let tx = self.store.begin_read()?;
        let mut entries = Vec::new();
        for (key, bytes) in tx.scan_prefix(Table::Enrollments, &[])? {
            let record = decode_record(&*tx, &key, &bytes)?;
            if key.as_slice() != record.enrollment.id.as_bytes() {
                return Err(StorageError::Corrupt {
                    what: "ключ регистрации не совпадает с записью",
                });
            }
            if record.enrollment.authenticator != authenticator {
                continue;
            }
            let index = crate::redb::codec::enrollment_index_key(&record);
            if tx.get(Table::EnrollmentIndex, &index)?.as_deref()
                != Some(record.enrollment.id.as_bytes().as_slice())
            {
                return Err(StorageError::Corrupt {
                    what: "регистрация отсутствует в индексе",
                });
            }
            entries.push(crate::projection::RegistryEntry::from_record(&record));
        }
        for (key, value) in tx.scan_prefix(Table::EnrollmentIndex, &[])? {
            if key.len() != crate::limits::ENROLLMENT_INDEX_KEY_LEN
                || value.as_slice() != &key[crate::limits::SCOPE_LOCK_KEY_LEN..]
            {
                return Err(StorageError::Corrupt {
                    what: "неверный индекс регистрации",
                });
            }
            if &key[32..48] != authenticator.as_bytes() {
                continue;
            }
            let bytes = tx
                .get(Table::Enrollments, &value)?
                .ok_or(StorageError::Corrupt {
                    what: "индекс без регистрации",
                })?;
            let record = decode_record(&*tx, &value, &bytes)?;
            if crate::redb::codec::enrollment_index_key(&record).as_slice() != key.as_slice() {
                return Err(StorageError::Corrupt {
                    what: "индекс не соответствует регистрации",
                });
            }
        }
        entries.sort_by_key(|entry| (entry.enrollment.id, entry.enrollment.generation));
        if entries.len() > crate::mutation_plan::SNAPSHOTS_MAX {
            return Err(StorageError::LimitExceeded {
                limit: "снимок аппарата",
                actual: entries.len(),
                allowed: crate::mutation_plan::SNAPSHOTS_MAX,
            });
        }
        Ok(entries)
    }
}
