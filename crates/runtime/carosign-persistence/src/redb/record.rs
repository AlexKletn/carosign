//! The log entry code.
//!
//! ⚠️ The format is **temporary straight**: fields are written in a row of fixed lengths,
//! without protobuf. It is suitable for the logic of the journal and for checking the journal
//! itself, but **not** for compatibility between issues: the present scheme is a protobuf of
//! `carosign.storage.v1` 23), and its appearance is obliged to change the version of the scheme
//! with migration.
//!
//! The length of the fields is fixed, so the record does not depend on the `String` and the
//! size of the structure.

use crate::model::{MutationPhase, MutationRecord, StorageError};
use crate::redb::codec::{Envelope, SCHEMA_VERSION};
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, CommandId, MutationId, OperationId, SubjectId, WorkerInstanceId,
};

/// A sign of "no field."
const ABSENT: u8 = 0;
/// A sign of a field.
const PRESENT: u8 = 1;

fn phase_code(phase: MutationPhase) -> u8 {
    match phase {
        MutationPhase::Prepared => 1,
        MutationPhase::DispatchArmed => 2,
        MutationPhase::ProviderObserved => 3,
        MutationPhase::RegistryCommitted => 4,
        MutationPhase::Completed => 5,
        MutationPhase::NeedsReconciliation => 6,
        MutationPhase::NoChange => 7,
    }
}

fn phase_from(code: u8) -> Result<MutationPhase, StorageError> {
    Ok(match code {
        1 => MutationPhase::Prepared,
        2 => MutationPhase::DispatchArmed,
        3 => MutationPhase::ProviderObserved,
        4 => MutationPhase::RegistryCommitted,
        5 => MutationPhase::Completed,
        6 => MutationPhase::NeedsReconciliation,
        7 => MutationPhase::NoChange,
        _ => {
            return Err(StorageError::Corrupt {
                what: "фаза изменения",
            });
        }
    })
}

/// Collects a magazine entry into a shell.
pub fn encode_mutation(record: &MutationRecord) -> Result<Vec<u8>, StorageError> {
    if record.provider_observation_digest.is_some() || record.terminal_outcome.is_some() {
        // The temporal format does not store these fields: you can not lose them when
        // writing.
        return Err(StorageError::Unavailable);
    }
    record.validate().map_err(corrupt)?;
    record
        .plan
        .as_ref()
        .ok_or(StorageError::Corrupt {
            what: "нет плана изменения",
        })?
        .validate(crate::mutation_plan::Scope {
            subject: record.subject,
            authenticator: record.authenticator,
        })
        .map_err(corrupt)?;
    let reference = record
        .recovery_ref
        .as_ref()
        .map(|value| value.as_bytes())
        .unwrap_or(&[]);
    let mut payload = Vec::with_capacity(128 + reference.len());

    payload.extend_from_slice(record.mutation.as_bytes());
    payload.extend_from_slice(record.operation.as_bytes());
    payload.extend_from_slice(record.subject.authority.as_bytes());
    payload.extend_from_slice(record.subject.subject.as_bytes());
    payload.extend_from_slice(record.authenticator.as_bytes());
    payload.push(phase_code(record.phase));
    payload.push(if record.expected_generation.is_some() {
        PRESENT
    } else {
        ABSENT
    });
    payload.extend_from_slice(
        &record
            .expected_generation
            .map(|generation| generation.value())
            .unwrap_or(0)
            .to_be_bytes(),
    );
    payload.push(if record.worker.is_some() {
        PRESENT
    } else {
        ABSENT
    });
    payload.extend_from_slice(
        record
            .worker
            .map(|value| *value.as_bytes())
            .unwrap_or([0u8; 16])
            .as_slice(),
    );
    payload.push(if record.epoch.is_some() {
        PRESENT
    } else {
        ABSENT
    });
    payload.extend_from_slice(
        &record
            .epoch
            .map(|epoch| epoch.value())
            .unwrap_or(0)
            .to_be_bytes(),
    );
    payload.push(if record.command.is_some() {
        PRESENT
    } else {
        ABSENT
    });
    payload.extend_from_slice(
        record
            .command
            .map(|value| *value.as_bytes())
            .unwrap_or([0u8; 16])
            .as_slice(),
    );
    payload.push(if record.recovery_ref.is_some() {
        PRESENT
    } else {
        ABSENT
    });
    payload.extend_from_slice(&(reference.len() as u32).to_be_bytes());
    payload.extend_from_slice(reference);

    // ⚠️ Change Plan (Document 33): length + canonical bytes.
    let plan = record
        .plan
        .as_ref()
        .map(crate::mutation_plan::encode_plan)
        .transpose()
        .map_err(|_| StorageError::Corrupt {
            what: "план изменения не кодируется",
        })?
        .unwrap_or_default();
    payload.push(if plan.is_empty() { ABSENT } else { PRESENT });
    payload.extend_from_slice(&(plan.len() as u32).to_be_bytes());
    payload.extend_from_slice(&plan);

    Envelope {
        schema: SCHEMA_VERSION,
        payload,
    }
    .encode()
}

/// Deconstructing the log entry.
pub fn decode_mutation(bytes: &[u8]) -> Result<MutationRecord, StorageError> {
    let envelope = Envelope::decode(bytes)?;
    if envelope.schema != SCHEMA_VERSION {
        return Err(StorageError::UnsupportedSchema {
            found: envelope.schema,
        });
    }
    let mut cursor = Cursor::new(&envelope.payload);

    let mutation = MutationId::from_bytes(cursor.take_16()?).map_err(corrupt)?;
    let operation = OperationId::from_bytes(cursor.take_16()?).map_err(corrupt)?;
    let authority = AuthorityId::from_bytes(cursor.take_16()?).map_err(corrupt)?;
    let subject_id = SubjectId::from_bytes(cursor.take_16()?).map_err(corrupt)?;
    let authenticator = AuthenticatorId::from_bytes(cursor.take_16()?).map_err(corrupt)?;
    let phase = phase_from(cursor.take_1()?)?;

    let expected_generation = if cursor.take_presence()? {
        Some(EnrollmentGeneration::new(cursor.take_u64()?).map_err(corrupt)?)
    } else {
        if cursor.take_u64()? != 0 {
            return Err(StorageError::Corrupt {
                what: "отсутствующее поколение не равно нулю",
            });
        }
        None
    };
    let worker = if cursor.take_presence()? {
        Some(WorkerInstanceId::from_bytes(cursor.take_16()?).map_err(corrupt)?)
    } else {
        if cursor.take_16()? != [0; 16] {
            return Err(StorageError::Corrupt {
                what: "отсутствующий работник не равен нулю",
            });
        }
        None
    };
    let epoch = if cursor.take_presence()? {
        Some(DeviceEpoch::new(cursor.take_u64()?).map_err(corrupt)?)
    } else {
        if cursor.take_u64()? != 0 {
            return Err(StorageError::Corrupt {
                what: "отсутствующая эпоха не равна нулю",
            });
        }
        None
    };
    let command = if cursor.take_presence()? {
        Some(CommandId::from_bytes(cursor.take_16()?).map_err(corrupt)?)
    } else {
        if cursor.take_16()? != [0; 16] {
            return Err(StorageError::Corrupt {
                what: "отсутствующая команда не равна нулю",
            });
        }
        None
    };
    let has_ref = cursor.take_presence()?;
    let ref_len = cursor.take_u32()? as usize;
    if !has_ref && ref_len != 0 {
        return Err(StorageError::Corrupt {
            what: "длина отсутствующей ссылки восстановления",
        });
    }
    if !cursor.remaining(ref_len) {
        return Err(StorageError::Corrupt {
            what: "ссылка восстановления",
        });
    }
    let recovery_ref = if has_ref {
        Some(OpaqueProviderRef::new(cursor.take(ref_len)?.to_vec()).map_err(corrupt)?)
    } else {
        None
    };

    // ⚠️ The plan is mandatory: the recording without it is spoilage, not “almost ready”.
    let has_plan = cursor.take_presence()?;
    let plan_len = cursor.take_u32()? as usize;
    if !has_plan && plan_len != 0 {
        return Err(StorageError::Corrupt {
            what: "длина отсутствующего плана",
        });
    }
    if !cursor.remaining(plan_len) {
        return Err(StorageError::Corrupt {
            what: "план изменения обрезан",
        });
    }
    let plan = if has_plan {
        Some(
            crate::mutation_plan::decode_plan(cursor.take(plan_len)?).map_err(|_| {
                StorageError::Corrupt {
                    what: "план изменения не разбирается",
                }
            })?,
        )
    } else {
        None
    };
    if cursor.remaining_left() != 0 {
        return Err(StorageError::Corrupt {
            what: "лишние байты записи",
        });
    }

    let record = MutationRecord {
        mutation,
        operation,
        subject: SubjectRef {
            authority,
            subject: subject_id,
        },
        authenticator,
        expected_generation,
        phase,
        worker,
        epoch,
        command,
        recovery_ref,
        provider_observation_digest: None,
        terminal_outcome: None,
        plan,
    };
    record.validate().map_err(corrupt)?;
    record
        .plan
        .as_ref()
        .ok_or(StorageError::Corrupt {
            what: "нет плана изменения",
        })?
        .validate(crate::mutation_plan::Scope {
            subject: record.subject,
            authenticator: record.authenticator,
        })
        .map_err(corrupt)?;
    if matches!(
        record.phase,
        MutationPhase::DispatchArmed
            | MutationPhase::ProviderObserved
            | MutationPhase::RegistryCommitted
    ) && (record.worker.is_none() || record.epoch.is_none() || record.command.is_none())
    {
        return Err(StorageError::Corrupt {
            what: "неполная привязка отправленного изменения",
        });
    }
    Ok(record)
}

fn corrupt<E>(error: E) -> StorageError {
    let _ = error;
    StorageError::Corrupt {
        what: "поле записи журнала",
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn remaining(&self, count: usize) -> bool {
        self.data.len().saturating_sub(self.at) >= count
    }

    fn remaining_left(&self) -> usize {
        self.data.len().saturating_sub(self.at)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], StorageError> {
        if !self.remaining(count) {
            return Err(StorageError::Corrupt {
                what: "запись кончилась раньше",
            });
        }
        let out = &self.data[self.at..self.at + count];
        self.at += count;
        Ok(out)
    }

    fn take_1(&mut self) -> Result<u8, StorageError> {
        Ok(self.take(1)?[0])
    }

    fn take_presence(&mut self) -> Result<bool, StorageError> {
        match self.take_1()? {
            ABSENT => Ok(false),
            PRESENT => Ok(true),
            _ => Err(StorageError::Corrupt {
                what: "неизвестный признак наличия поля",
            }),
        }
    }

    fn take_16(&mut self) -> Result<[u8; 16], StorageError> {
        let bytes = self.take(16)?;
        let mut out = [0u8; 16];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn take_u32(&mut self) -> Result<u32, StorageError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn take_u64(&mut self) -> Result<u64, StorageError> {
        let bytes = self.take(8)?;
        let mut out = [0u8; 8];
        out.copy_from_slice(bytes);
        Ok(u64::from_be_bytes(out))
    }
}

#[path = "record.tests.rs"]
#[cfg(test)]
mod tests;
