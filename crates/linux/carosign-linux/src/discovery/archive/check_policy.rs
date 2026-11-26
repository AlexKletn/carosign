use super::super::supervisor_policy::SupervisorPolicy;
use super::super::verified_installation::InstallationMetadataDecoder;
use super::ArchiveError;
use super::CheckedArchive;
use super::safe_path;
use std::collections::BTreeSet;

/// The verification of the administrative policy does not confirm its origin.
/// The caller must obtain the policy from a trusted system source.
pub(in crate::discovery) fn check_policy(
    archive: &CheckedArchive,
    decoder: &dyn InstallationMetadataDecoder,
    policy: &SupervisorPolicy,
) -> Result<(), ArchiveError> {
    let manifest = archive
        .files
        .get("package.toml")
        .ok_or(ArchiveError::InvalidMetadata)?;
    let manifest = std::str::from_utf8(manifest).map_err(|_| ArchiveError::InvalidMetadata)?;
    let manifest = decoder
        .manifest(manifest, &policy.profile)
        .map_err(|_| ArchiveError::InvalidMetadata)?;
    if manifest.package != policy.package
        || manifest.version != policy.version
        || manifest.executable != policy.executable
    {
        return Err(ArchiveError::InvalidMetadata);
    }
    let mut expected = BTreeSet::new();
    for path in [
        "package.toml",
        manifest.profile_path.as_str(),
        policy.executable.as_str(),
    ]
    .into_iter()
    .chain(policy.runtime.iter().map(String::as_str))
    {
        safe_path(path.as_bytes(), false)?;
        if !expected.insert(path) || !archive.files.contains_key(path) {
            return Err(ArchiveError::InvalidMetadata);
        }
    }
    let profile = archive
        .files
        .get(&manifest.profile_path)
        .ok_or(ArchiveError::InvalidMetadata)?;
    let profile = std::str::from_utf8(profile).map_err(|_| ArchiveError::InvalidMetadata)?;
    if decoder
        .profile(profile)
        .map_err(|_| ArchiveError::InvalidMetadata)?
        != policy.profile
        || expected.len() != archive.files.len()
    {
        return Err(ArchiveError::InvalidMetadata);
    }
    Ok(())
}
