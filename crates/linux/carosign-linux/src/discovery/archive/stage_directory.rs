pub(super) struct StageDirectory(pub(super) std::path::PathBuf);
impl Drop for StageDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
