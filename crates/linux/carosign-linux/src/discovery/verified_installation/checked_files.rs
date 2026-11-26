use std::io::Read;
use std::os::fd::OwnedFd;

use carosign_core::ids::{PackageId, ProfileId, ProviderId};

use crate::discovery::supervisor_policy::SupervisorPolicy;
use crate::sys;

use super::{
    ApprovedInstallation, Authority, FileRole, InstallationKey, InstallationMetadataDecoder,
    KernelVerity, Measure, Refusal, UnavailableAuthority, approved_path::approved_path,
    pinned_file::pinned_file, protected::protected,
};

/// It is not a start-up authorization or evidence of process limitations.
pub(in crate::discovery) struct CheckedFiles {
    policy: SupervisorPolicy,
    executable: OwnedFd,
    runtime: Vec<(String, OwnedFd)>,
    root: OwnedFd,
    policy_file: OwnedFd,
    manifest_file: OwnedFd,
    profile_file: OwnedFd,
    record: ApprovedInstallation,
    owner: u32,
}

impl CheckedFiles {
    /// Reconfirms the retained objects and the source before transferring the launch.
    /// The call does not replace the serialization of the exec itself with revocation powers.
    fn revalidate(&self, authority: &dyn Authority, measure: &dyn Measure) -> Result<(), Refusal> {
        authority.revalidate(&self.record)?;
        if authority.approved(self.record.key)? != self.record {
            return Err(Refusal::Changed);
        }
        let root = sys::launch_metadata(&self.root).map_err(|e| Refusal::Kernel(e.errno))?;
        if !protected(&root, self.owner, true) || root.identity != self.record.root {
            return Err(Refusal::Changed);
        }
        let files = [
            (
                &self.record.policy_path,
                FileRole::Policy,
                &self.policy_file,
            ),
            (
                &self.record.manifest_path,
                FileRole::Manifest,
                &self.manifest_file,
            ),
            (
                &self.record.profile_path,
                FileRole::Profile,
                &self.profile_file,
            ),
            (
                &self.record.executable_path,
                FileRole::Executable,
                &self.executable,
            ),
        ];
        for (path, role, fd) in files.into_iter().chain(
            self.runtime
                .iter()
                .map(|(path, fd)| (path, FileRole::Runtime, fd)),
        ) {
            let expected = self.record.files.get(path).ok_or(Refusal::Mismatch)?;
            let metadata = sys::launch_metadata(fd).map_err(|e| Refusal::Kernel(e.errno))?;
            if expected.role != role
                || !protected(&metadata, self.owner, false)
                || (role == FileRole::Executable && metadata.mode & 0o111 == 0)
                || !measure.matches(fd, &expected.digest)?
            {
                return Err(Refusal::Mismatch);
            }
            // A fixed object does not, by itself, confirm that the path to it and its
            // parental catalogs remain secure: Re-allow the path from the retained root
            // without reference, and compare the final object with the one that was tested in
            // preparation.
            let current = pinned_file(&self.root, path, self.owner)?;
            let current_meta =
                sys::launch_metadata(&current).map_err(|e| Refusal::Kernel(e.errno))?;
            if current_meta != metadata {
                return Err(Refusal::Changed);
            }
        }
        authority.revalidate(&self.record)?;
        if authority.approved(self.record.key)? != self.record {
            return Err(Refusal::Changed);
        }
        Ok(())
    }
}

fn verified_file(
    root: &OwnedFd,
    record: &ApprovedInstallation,
    path: &str,
    role: FileRole,
    owner: u32,
    measure: &dyn Measure,
) -> Result<OwnedFd, Refusal> {
    let expected = record.files.get(path).ok_or(Refusal::Mismatch)?;
    if expected.role != role {
        return Err(Refusal::Mismatch);
    }
    let file = pinned_file(root, path, owner)?;
    if !measure.matches(&file, &expected.digest)? {
        return Err(Refusal::Mismatch);
    }
    Ok(file)
}

fn text(file: &OwnedFd) -> Result<String, Refusal> {
    let mut bytes = Vec::new();
    std::fs::File::from(file.try_clone().map_err(|_| Refusal::Unavailable)?)
        .take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| Refusal::Unavailable)?;
    if bytes.len() > 65_536 {
        return Err(Refusal::InvalidData);
    }
    String::from_utf8(bytes).map_err(|_| Refusal::InvalidData)
}

pub(super) fn validate(
    authority: &dyn Authority,
    measure: &dyn Measure,
    decoder: &dyn InstallationMetadataDecoder,
    key: InstallationKey,
    root: OwnedFd,
    owner: u32,
) -> Result<CheckedFiles, Refusal> {
    let record = authority.approved(key)?;
    if record.key != key {
        return Err(Refusal::Mismatch);
    }
    if record.revision == 0
        || record.key_policy_id == [0; 16]
        || !matches!(
            (record.signer_key_version, record.signer_fingerprint.len()),
            (4, 20) | (5 | 6, 32)
        )
        || record.signer_fingerprint.iter().all(|byte| *byte == 0)
        || !approved_path(&record.version_dir)
        || record.files.len() > 128
        || record.files.keys().any(|path| !approved_path(path))
        || [
            &record.policy_path,
            &record.manifest_path,
            &record.profile_path,
            &record.executable_path,
        ]
        .iter()
        .any(|path| !approved_path(path))
    {
        return Err(Refusal::InvalidData);
    }
    let root_meta = sys::launch_metadata(&root).map_err(|e| Refusal::Kernel(e.errno))?;
    if !protected(&root_meta, owner, true) {
        return Err(Refusal::UnsafeObject);
    }
    if root_meta.identity != record.root {
        return Err(Refusal::Changed);
    }

    let policy_file = verified_file(
        &root,
        &record,
        &record.policy_path,
        FileRole::Policy,
        owner,
        measure,
    )?;
    let policy = SupervisorPolicy::parse(text(&policy_file)?.as_bytes())
        .map_err(|_| Refusal::InvalidData)?;
    let manifest_file = verified_file(
        &root,
        &record,
        &record.manifest_path,
        FileRole::Manifest,
        owner,
        measure,
    )?;
    let manifest = decoder
        .manifest(&text(&manifest_file)?, &record.profile)
        .map_err(|_| Refusal::InvalidData)?;
    if policy.package != record.package
        || policy.version != record.version
        || policy.profile != record.profile
        || manifest.package != record.package
        || manifest.version != record.version
        || manifest.executable != policy.executable
        || record.executable_path != policy.executable
        || record.profile_path != manifest.profile_path
    {
        return Err(Refusal::Mismatch);
    }
    let profile_file = verified_file(
        &root,
        &record,
        &manifest.profile_path,
        FileRole::Profile,
        owner,
        measure,
    )?;
    let profile = decoder
        .profile(&text(&profile_file)?)
        .map_err(|_| Refusal::InvalidData)?;
    if profile != record.profile {
        return Err(Refusal::Mismatch);
    }
    let executable = verified_file(
        &root,
        &record,
        &policy.executable,
        FileRole::Executable,
        owner,
        measure,
    )?;
    let meta = sys::launch_metadata(&executable).map_err(|e| Refusal::Kernel(e.errno))?;
    if meta.mode & 0o111 == 0 {
        return Err(Refusal::UnsafeObject);
    }
    let mut runtime = Vec::with_capacity(policy.runtime.len());
    for path in &policy.runtime {
        runtime.push((
            path.clone(),
            verified_file(&root, &record, path, FileRole::Runtime, owner, measure)?,
        ));
    }
    // References to the same path in different roles do not replace verification.
    let mut used = std::collections::BTreeSet::new();
    for path in [
        &record.policy_path,
        &record.manifest_path,
        &manifest.profile_path,
        &policy.executable,
    ]
    .into_iter()
    .chain(policy.runtime.iter())
    {
        if !used.insert(path) {
            return Err(Refusal::Mismatch);
        }
    }
    if used.len() != record.files.len() {
        return Err(Refusal::Mismatch);
    }
    authority.revalidate(&record)?;
    // Re-reading must match the entire shot, including roles and dimensions, and is not a
    // substitute for re-checking before exec itself.
    if authority.approved(key)? != record {
        return Err(Refusal::Changed);
    }
    Ok(CheckedFiles {
        policy,
        executable,
        runtime,
        root,
        policy_file,
        manifest_file,
        profile_file,
        record,
        owner,
    })
}

/// System validation uses only the actual source of kernel installation and measurement.
/// Until the source is connected, it returns. `Unavailable`.
pub(in crate::discovery) fn system_validate(
    package: PackageId,
    provider: ProviderId,
    profile: ProfileId,
    root: OwnedFd,
    decoder: &dyn InstallationMetadataDecoder,
) -> Result<CheckedFiles, Refusal> {
    validate(
        &UnavailableAuthority,
        &KernelVerity,
        decoder,
        InstallationKey {
            package,
            provider,
            profile,
        },
        root,
        0,
    )
}

#[cfg(test)]
#[path = "checked_files.tests.rs"]
mod tests;
