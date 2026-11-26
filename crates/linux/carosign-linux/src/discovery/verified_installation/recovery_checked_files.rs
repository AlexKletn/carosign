use std::collections::BTreeMap;
use std::os::fd::OwnedFd;

use crate::sys::{self, FileIdentity};

use super::{
    KernelVerity, Measure, RecoveryFileCandidate, RecoveryFilesRefusal, Refusal,
    pinned_file::pinned_file, protected::protected,
};

/// Retained quarantine files; does not allow the launch or lifting of quarantine.
pub struct RecoveryCheckedFiles {
    root: OwnedFd,
    root_identity: FileIdentity,
    files: Vec<(RecoveryFileCandidate, OwnedFd)>,
}

impl RecoveryCheckedFiles {
    pub(in crate::discovery) fn matches_contents(
        &self,
        expected: &BTreeMap<String, Vec<u8>>,
    ) -> bool {
        self.files.len() == expected.len()
            && self.files.iter().all(|(candidate, fd)| {
                expected.get(&candidate.path).is_some_and(|bytes| {
                    crate::discovery::archive::matches_signed_bytes(fd, bytes).unwrap_or(false)
                })
            })
    }
    /// Checks protected paths and measurements again.
    pub fn revalidate(&self) -> Result<(), RecoveryFilesRefusal> {
        self.revalidate_with(0, &KernelVerity)
            .map_err(|_| RecoveryFilesRefusal)
    }

    fn revalidate_with(&self, owner: u32, measure: &dyn Measure) -> Result<(), Refusal> {
        let root = sys::launch_metadata(&self.root).map_err(|e| Refusal::Kernel(e.errno))?;
        if !protected(&root, owner, true) || root.identity != self.root_identity {
            return Err(Refusal::Changed);
        }
        for (candidate, fd) in &self.files {
            let metadata = sys::launch_metadata(fd).map_err(|e| Refusal::Kernel(e.errno))?;
            if !protected(&metadata, owner, false) || !measure.matches(fd, &candidate.digest)? {
                return Err(Refusal::Mismatch);
            }
            let reopened = pinned_file(&self.root, &candidate.path, owner)?;
            if sys::launch_metadata(&reopened).map_err(|e| Refusal::Kernel(e.errno))? != metadata {
                return Err(Refusal::Changed);
            }
        }
        Ok(())
    }
}

/// Checks the actual files regarding the unverified image.
/// The signature of the archive and the origin of these measurements require separate
/// verification.
pub fn check_recovery_files(
    root: OwnedFd,
    files: Vec<RecoveryFileCandidate>,
) -> Result<RecoveryCheckedFiles, RecoveryFilesRefusal> {
    check_recovery_files_with(root, files, 0, &KernelVerity).map_err(|_| RecoveryFilesRefusal)
}

fn check_recovery_files_with(
    root: OwnedFd,
    files: Vec<RecoveryFileCandidate>,
    owner: u32,
    measure: &dyn Measure,
) -> Result<RecoveryCheckedFiles, Refusal> {
    if files.is_empty() || files.len() > 128 {
        return Err(Refusal::InvalidData);
    }
    let metadata = sys::launch_metadata(&root).map_err(|e| Refusal::Kernel(e.errno))?;
    if !protected(&metadata, owner, true) {
        return Err(Refusal::UnsafeObject);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut checked = Vec::with_capacity(files.len());
    for candidate in files {
        if candidate.digest == [0; 32] || !seen.insert(candidate.path.clone()) {
            return Err(Refusal::InvalidData);
        }
        let fd = pinned_file(&root, &candidate.path, owner)?;
        if !measure.matches(&fd, &candidate.digest)? {
            return Err(Refusal::Mismatch);
        }
        checked.push((candidate, fd));
    }
    let result = RecoveryCheckedFiles {
        root,
        root_identity: metadata.identity,
        files: checked,
    };
    result.revalidate_with(owner, measure)?;
    Ok(result)
}

#[cfg(test)]
#[path = "recovery_checked_files.tests.rs"]
mod tests;
