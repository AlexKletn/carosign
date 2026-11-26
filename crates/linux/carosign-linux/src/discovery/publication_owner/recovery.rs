use std::collections::BTreeMap;
use std::path::Path;

use crate::discovery::{archive, supervisor_policy::SupervisorPolicy};
use crate::policy::{PolicyChangeWatch, PolicyWatchState, ProtectedPolicySnapshot};

use super::{
    InstallationMetadataDecoder, PublicationFileRole, QuarantineRefusal, RecoveryCheckedFiles,
    RecoveryExpected, ReverifiedProvenance, STAGED_POLICY_PATH,
    publication_file_role::publication_file_role,
};

/// Overseeing administrative policies checked along with the content.
pub struct ReverifiedContents {
    watch: PolicyChangeWatch,
}

impl ReverifiedContents {
    pub fn revalidate(&mut self) -> Result<(), QuarantineRefusal> {
        if self.watch.check().map_err(|_| QuarantineRefusal)? != PolicyWatchState::Unchanged {
            return Err(QuarantineRefusal);
        }
        Ok(())
    }
}

/// Verifies signed archive, current protected policy and withheld files.
/// Confirmation does not allow launch and does not substitute for recall serialization.
pub fn reverify_contents(
    provenance: &mut ReverifiedProvenance,
    files: &RecoveryCheckedFiles,
    policy_path: &Path,
    decoder: &dyn InstallationMetadataDecoder,
    expected: RecoveryExpected<'_>,
) -> Result<ReverifiedContents, QuarantineRefusal> {
    provenance.revalidate()?;
    files.revalidate().map_err(|_| QuarantineRefusal)?;
    let (contents, watch) = ProtectedPolicySnapshot::open(policy_path)
        .map_err(|_| QuarantineRefusal)?
        .into_parts();
    let policy = SupervisorPolicy::parse(&contents).map_err(|_| QuarantineRefusal)?;
    let mut result = ReverifiedContents { watch };
    result.revalidate()?;
    if policy.package != expected.package
        || policy.version != expected.version
        || policy.profile != expected.profile
        || policy.executable != expected.executable_path
    {
        return Err(QuarantineRefusal);
    }
    let mut archive = archive::inspect(provenance.artifact(), decoder, &policy.profile)
        .map_err(|_| QuarantineRefusal)?;
    archive::check_policy(&archive, decoder, &policy).map_err(|_| QuarantineRefusal)?;
    let manifest = std::str::from_utf8(archive.files.get("package.toml").ok_or(QuarantineRefusal)?)
        .map_err(|_| QuarantineRefusal)?;
    let manifest = decoder
        .manifest(manifest, &policy.profile)
        .map_err(|_| QuarantineRefusal)?;
    if manifest.profile_path != expected.profile_path {
        return Err(QuarantineRefusal);
    }
    if archive
        .files
        .insert(STAGED_POLICY_PATH.to_owned(), contents)
        .is_some()
    {
        return Err(QuarantineRefusal);
    }
    check_recovery_roles(
        &archive.files,
        &manifest.profile_path,
        &policy.executable,
        &policy.runtime,
        expected.files,
    )?;
    if !files.matches_contents(&archive.files) {
        return Err(QuarantineRefusal);
    }
    files.revalidate().map_err(|_| QuarantineRefusal)?;
    result.revalidate()?;
    provenance.revalidate()?;
    Ok(result)
}

fn check_recovery_roles(
    files: &BTreeMap<String, Vec<u8>>,
    profile: &str,
    executable: &str,
    runtime: &[String],
    expected: &[(String, PublicationFileRole)],
) -> Result<(), QuarantineRefusal> {
    if files.len() != expected.len() {
        return Err(QuarantineRefusal);
    }
    let mut seen = std::collections::BTreeSet::new();
    for (path, role) in expected {
        if !seen.insert(path)
            || !files.contains_key(path)
            || publication_file_role(path, profile, executable, runtime).as_ref() != Some(role)
        {
            return Err(QuarantineRefusal);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "recovery.tests.rs"]
mod tests;
