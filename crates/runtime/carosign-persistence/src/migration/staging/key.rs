use super::{InvalidStageRecord, TableKind};

#[derive(Debug, PartialEq, Eq)]
pub(in super::super) struct Key<'a> {
    pub migration: [u8; 16],
    pub table: TableKind,
    pub source_key: &'a [u8],
}

impl<'a> Key<'a> {
    pub(in super::super) fn decode(bytes: &'a [u8]) -> Result<Self, InvalidStageRecord> {
        if bytes.len() < 21 {
            return Err(InvalidStageRecord);
        }
        let migration: [u8; 16] = bytes[..16].try_into().map_err(|_| InvalidStageRecord)?;
        let table = TableKind::try_from(bytes[16])?;
        let len =
            u32::from_be_bytes(bytes[17..21].try_into().map_err(|_| InvalidStageRecord)?) as usize;
        if migration == [0; 16] || len != bytes.len() - 21 || !table.key_length_valid(&bytes[21..])
        {
            return Err(InvalidStageRecord);
        }
        Ok(Self {
            migration,
            table,
            source_key: &bytes[21..],
        })
    }

    pub(in super::super) fn encode(
        migration: [u8; 16],
        table: TableKind,
        source_key: &[u8],
    ) -> Result<Vec<u8>, InvalidStageRecord> {
        if migration == [0; 16] || !table.key_length_valid(source_key) {
            return Err(InvalidStageRecord);
        }
        let len = u32::try_from(source_key.len()).map_err(|_| InvalidStageRecord)?;
        let mut bytes = Vec::with_capacity(21 + source_key.len());
        bytes.extend_from_slice(&migration);
        bytes.push(table as u8);
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(source_key);
        Ok(bytes)
    }
}
