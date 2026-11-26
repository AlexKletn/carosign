use super::{InvalidStageRecord, TableKind};

#[derive(Debug, PartialEq, Eq)]
pub(in super::super) struct Value<'a> {
    pub source_digest: [u8; 32],
    pub prepared_value: &'a [u8],
}

impl<'a> Value<'a> {
    pub(in super::super) fn decode(
        bytes: &'a [u8],
        table: TableKind,
    ) -> Result<Self, InvalidStageRecord> {
        if bytes.len() < 36 {
            return Err(InvalidStageRecord);
        }
        let source_digest = bytes[..32].try_into().map_err(|_| InvalidStageRecord)?;
        let len =
            u32::from_be_bytes(bytes[32..36].try_into().map_err(|_| InvalidStageRecord)?) as usize;
        if len == 0 || len > table.max_value() || len != bytes.len() - 36 {
            return Err(InvalidStageRecord);
        }
        Ok(Self {
            source_digest,
            prepared_value: &bytes[36..],
        })
    }
    pub(in super::super) fn encode(
        table: TableKind,
        source_digest: [u8; 32],
        prepared_value: &[u8],
    ) -> Result<Vec<u8>, InvalidStageRecord> {
        if prepared_value.is_empty() || prepared_value.len() > table.max_value() {
            return Err(InvalidStageRecord);
        }
        let len = u32::try_from(prepared_value.len()).map_err(|_| InvalidStageRecord)?;
        let mut bytes = Vec::with_capacity(36 + prepared_value.len());
        bytes.extend_from_slice(&source_digest);
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(prepared_value);
        Ok(bytes)
    }
}
