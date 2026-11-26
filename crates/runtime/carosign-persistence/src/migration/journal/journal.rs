use super::{ENVELOPE_LEN, InvalidJournal, PAYLOAD_LEN, Snapshot, Stage, sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) struct Journal {
    pub id: [u8; 16],
    pub backup_length: u64,
    pub backup_digest: [u8; 32],
    pub backup_device: u64,
    pub backup_inode: u64,
    pub stage: Stage,
}

impl Journal {
    pub(super) fn valid(&self) -> bool {
        let common = self.id != [0; 16]
            && self.backup_length != 0
            && self.backup_digest != [0; 32]
            && self.backup_inode != 0;
        let snapshot = match self.stage {
            Stage::Pending => true,
            Stage::Staged(snapshot) | Stage::Committed(snapshot) => {
                snapshot.digest != [0; 32]
                    && (snapshot.count != 0 || snapshot.digest == sha256(&[]))
            }
        };
        common && snapshot
    }

    /// Encodes only the permissible combination of fields and stages.
    pub(in super::super) fn encode(&self) -> Result<Vec<u8>, InvalidJournal> {
        if !self.valid() {
            return Err(InvalidJournal);
        }
        let mut payload = [0u8; PAYLOAD_LEN];
        payload[..4].copy_from_slice(b"BMIG");
        payload[4] = 1;
        payload[5] = match self.stage {
            Stage::Pending => 1,
            Stage::Staged(_) => 2,
            Stage::Committed(_) => 3,
        };
        payload[8..12].copy_from_slice(&3u32.to_be_bytes());
        payload[12..16].copy_from_slice(&4u32.to_be_bytes());
        payload[16..32].copy_from_slice(&self.id);
        payload[32..40].copy_from_slice(&self.backup_length.to_be_bytes());
        payload[40..72].copy_from_slice(&self.backup_digest);
        payload[72..80].copy_from_slice(&self.backup_device.to_be_bytes());
        payload[80..88].copy_from_slice(&self.backup_inode.to_be_bytes());
        if let Stage::Staged(snapshot) | Stage::Committed(snapshot) = self.stage {
            payload[88..96].copy_from_slice(&snapshot.count.to_be_bytes());
            payload[96..128].copy_from_slice(&snapshot.digest);
        }
        let mut result = Vec::with_capacity(ENVELOPE_LEN);
        result.extend_from_slice(b"BIO1");
        result.extend_from_slice(&(PAYLOAD_LEN as u32).to_le_bytes());
        result.extend_from_slice(&payload);
        result.extend_from_slice(&sha256(&payload));
        Ok(result)
    }

    /// Checks the shell, checksum and all format limitations.
    pub(in super::super) fn decode(bytes: &[u8]) -> Result<Self, InvalidJournal> {
        if bytes.len() != ENVELOPE_LEN
            || &bytes[..4] != b"BIO1"
            || bytes[4..8] != (PAYLOAD_LEN as u32).to_le_bytes()
        {
            return Err(InvalidJournal);
        }
        let payload = &bytes[8..8 + PAYLOAD_LEN];
        if bytes[8 + PAYLOAD_LEN..] != sha256(payload)
            || &payload[..4] != b"BMIG"
            || payload[4] != 1
            || payload[6..8] != [0; 2]
            || payload[8..12] != 3u32.to_be_bytes()
            || payload[12..16] != 4u32.to_be_bytes()
        {
            return Err(InvalidJournal);
        }
        let count = u64::from_be_bytes(payload[88..96].try_into().map_err(|_| InvalidJournal)?);
        let digest = payload[96..128].try_into().map_err(|_| InvalidJournal)?;
        let stage = match payload[5] {
            1 if count == 0 && digest == [0; 32] => Stage::Pending,
            2 => Stage::Staged(Snapshot { count, digest }),
            3 => Stage::Committed(Snapshot { count, digest }),
            _ => return Err(InvalidJournal),
        };
        let journal = Self {
            id: payload[16..32].try_into().map_err(|_| InvalidJournal)?,
            backup_length: u64::from_be_bytes(
                payload[32..40].try_into().map_err(|_| InvalidJournal)?,
            ),
            backup_digest: payload[40..72].try_into().map_err(|_| InvalidJournal)?,
            backup_device: u64::from_be_bytes(
                payload[72..80].try_into().map_err(|_| InvalidJournal)?,
            ),
            backup_inode: u64::from_be_bytes(
                payload[80..88].try_into().map_err(|_| InvalidJournal)?,
            ),
            stage,
        };
        if !journal.valid() {
            return Err(InvalidJournal);
        }
        Ok(journal)
    }

    /// Verifies the legal stage change without changing the confirmed copy.
    pub(in super::super) fn may_advance_to(&self, next: &Self) -> bool {
        self.valid()
            && next.valid()
            && self.id == next.id
            && self.backup_length == next.backup_length
            && self.backup_digest == next.backup_digest
            && self.backup_device == next.backup_device
            && self.backup_inode == next.backup_inode
            && match (self.stage, next.stage) {
                (Stage::Pending, Stage::Staged(_)) => true,
                (Stage::Staged(before), Stage::Committed(after)) => before == after,
                _ => false,
            }
    }
}
