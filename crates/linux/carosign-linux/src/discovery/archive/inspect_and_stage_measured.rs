use super::super::signature_inputs::SignatureCheckedArtifact;
use super::super::supervisor_policy::SupervisorPolicy;
use super::super::verified_installation::InstallationMetadataDecoder;
use super::ArchiveError;
use super::check_policy;
use super::inspect;
use super::matches_signed_bytes;
use super::stage;
use super::stage_and_measure;
use std::collections::BTreeMap;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::OwnedFd;

/// Checks and prepares a picture that has passed the signature verification.
/// The result is not an installation or launch authorization: the caller must confirm the
/// origin of the key and the verifier, policy and record.
/// If the error is made, the caller destroys the closed directory entirely.
pub(in crate::discovery) fn inspect_and_stage_measured(
    artifact: &SignatureCheckedArtifact,
    decoder: &dyn InstallationMetadataDecoder,
    profile: &str,
    root: BorrowedFd<'_>,
) -> Result<BTreeMap<String, (OwnedFd, [u8; 32])>, ArchiveError> {
    let checked = inspect(artifact, decoder, profile)?;
    stage_and_measure(&checked, root)
}

/// Prepares a verified picture only after checking with the transmitted policy.
/// The authority to establish and originate the policy is confirmed separately.
pub(in crate::discovery) fn inspect_policy_and_stage_measured(
    artifact: &SignatureCheckedArtifact,
    decoder: &dyn InstallationMetadataDecoder,
    policy: &SupervisorPolicy,
    root: BorrowedFd<'_>,
) -> Result<BTreeMap<String, (OwnedFd, [u8; 32])>, ArchiveError> {
    let checked = inspect(artifact, decoder, &policy.profile)?;
    check_policy(&checked, decoder, policy)?;
    let files = stage(&checked, root)?;
    let executable = files
        .get(&policy.executable)
        .ok_or(ArchiveError::InvalidMetadata)?;
    // SAFETY: The descriptor belongs to the newly created closed directory file.
    if unsafe { libc::fchmod(executable.as_raw_fd(), 0o555) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // The rights of the executable file are part of the prepared installation.
    if unsafe { libc::fsync(executable.as_raw_fd()) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    use std::os::fd::AsFd;
    files
        .into_iter()
        .map(|(path, file)| {
            let digest = super::super::verity::enable_and_measure(file.as_fd())?;
            let expected = checked.files.get(&path).ok_or(ArchiveError::Unsafe)?;
            if !matches_signed_bytes(&file, expected)? {
                return Err(ArchiveError::Unsafe);
            }
            if unsafe { libc::fsync(file.as_raw_fd()) } < 0 {
                return Err(io::Error::last_os_error().into());
            }
            Ok((path, (file, digest)))
        })
        .collect()
}
