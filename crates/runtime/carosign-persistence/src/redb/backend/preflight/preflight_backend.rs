use super::{PREFLIGHT_WRITE_LIMIT, PreflightChanges, StorageBackend};

/// Redb opens a service header even without a written transaction.
/// To verify an unknown diagram, these records remain only in memory.
#[derive(Debug)]
pub(in super::super) struct PreflightBackend {
    base: std::sync::Arc<redb::backends::FileBackend>,
    changes: std::sync::Mutex<PreflightChanges>,
}

impl PreflightBackend {
    pub(in super::super) fn new(
        base: std::sync::Arc<redb::backends::FileBackend>,
    ) -> std::io::Result<Self> {
        let len = base.len()?;
        Ok(Self {
            base,
            changes: std::sync::Mutex::new(PreflightChanges {
                len,
                base_visible_len: len,
                bytes: Default::default(),
            }),
        })
    }
}

impl StorageBackend for PreflightBackend {
    fn len(&self) -> std::io::Result<u64> {
        Ok(self
            .changes
            .lock()
            .map_err(|_| std::io::Error::other("preflight lock"))?
            .len)
    }

    fn read(&self, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
        let changes = self
            .changes
            .lock()
            .map_err(|_| std::io::Error::other("preflight lock"))?;
        let end = offset
            .checked_add(len as u64)
            .ok_or_else(|| std::io::Error::other("preflight overflow"))?;
        if end > changes.len {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let available = changes
            .base_visible_len
            .saturating_sub(offset)
            .min(len as u64) as usize;
        let mut bytes = vec![0; len];
        if available != 0 {
            bytes[..available].copy_from_slice(&self.base.read(offset, available)?);
        }
        for (&position, &value) in changes.bytes.range(offset..end) {
            bytes[(position - offset) as usize] = value;
        }
        Ok(bytes)
    }

    fn set_len(&self, len: u64) -> std::io::Result<()> {
        let mut changes = self
            .changes
            .lock()
            .map_err(|_| std::io::Error::other("preflight lock"))?;
        changes.base_visible_len = changes.base_visible_len.min(len);
        changes.len = len;
        changes.bytes.split_off(&len);
        Ok(())
    }

    fn sync_data(&self, _eventual: bool) -> std::io::Result<()> {
        Ok(())
    }

    fn write(&self, offset: u64, bytes: &[u8]) -> std::io::Result<()> {
        let mut changes = self
            .changes
            .lock()
            .map_err(|_| std::io::Error::other("preflight lock"))?;
        let end = offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("preflight overflow"))?;
        if end > changes.len
            || bytes.len().saturating_add(changes.bytes.len()) > PREFLIGHT_WRITE_LIMIT
        {
            return Err(std::io::Error::other("preflight write limit"));
        }
        for (index, &byte) in bytes.iter().enumerate() {
            changes.bytes.insert(offset + index as u64, byte);
        }
        Ok(())
    }
}
