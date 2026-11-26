use std::process::Command;

pub(super) struct GpgHome(pub(super) std::path::PathBuf);
impl Drop for GpgHome {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/gpgconf")
            .arg("--homedir")
            .arg(&self.0)
            .args(["--kill", "gpg-agent"])
            .output();
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
