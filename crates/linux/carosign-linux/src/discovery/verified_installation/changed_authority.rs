use std::cell::Cell;

use super::{
    ApprovedInstallation, Authority, FileRole, InstallationKey, Refusal,
    installation_fixture::Fixture,
};

pub(super) struct ChangedAuthority<'a> {
    pub(super) fixture: &'a Fixture,
    pub(super) reads: Cell<usize>,
    pub(super) change: usize,
}
impl Authority for ChangedAuthority<'_> {
    fn approved(&self, _: InstallationKey) -> Result<ApprovedInstallation, Refusal> {
        let read = self.reads.get();
        self.reads.set(read + 1);
        let mut record = self.fixture.record.clone();
        if read > 0 {
            match self.change {
                0 => record.revision += 1,
                1 => record.signer_fingerprint[0] ^= 1,
                2 => record.key_policy_id[0] ^= 1,
                3 => record.version_dir.push('x'),
                4 => record.profile_path.push('x'),
                5 => record.executable_path.push('x'),
                6 => record.files.get_mut("worker").unwrap().digest[0] ^= 1,
                _ => record.files.get_mut("worker").unwrap().role = FileRole::Runtime,
            }
        }
        Ok(record)
    }
    fn revalidate(&self, _: &ApprovedInstallation) -> Result<(), Refusal> {
        Ok(())
    }
}
