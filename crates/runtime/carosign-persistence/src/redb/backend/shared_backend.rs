use super::StorageBackend;

/// Same locked file base after successful schema validation.
#[derive(Debug)]
pub(super) struct SharedBackend(pub(super) std::sync::Arc<redb::backends::FileBackend>);

impl StorageBackend for SharedBackend {
    fn len(&self) -> std::io::Result<u64> {
        self.0.len()
    }
    fn read(&self, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
        self.0.read(offset, len)
    }
    fn set_len(&self, len: u64) -> std::io::Result<()> {
        self.0.set_len(len)
    }
    fn sync_data(&self, eventual: bool) -> std::io::Result<()> {
        self.0.sync_data(eventual)
    }
    fn write(&self, offset: u64, data: &[u8]) -> std::io::Result<()> {
        self.0.write(offset, data)
    }
}
