use super::{
    AsFd, AsRawFd, InstallationFileRole, PublicationFileRole, QuarantineFailure, RecoveryExpected,
    RecoveryFileCandidate, RecoveryInputs, RecoverySnapshotSource, ReverifiedQuarantine,
    ReverifyProvenanceInputs, check_recovery_files, reverify_contents, reverify_provenance,
};

/// Independently double-checks the signature and fresh shot, without removing the quarantine.
///
/// # Safety
/// It requires a separate single-threaded assistant and an independently validated
/// installation root.
#[allow(dead_code)]
pub(crate) unsafe fn reverify_quarantine(
    store: &impl RecoverySnapshotSource,
    package_key: &[u8],
    installation_key: &[u8],
    inputs: RecoveryInputs<'_>,
) -> Result<ReverifiedQuarantine, QuarantineFailure> {
    inputs.root.revalidate()?;
    let snapshot = store
        .read_quarantine(package_key, installation_key)
        .map_err(|_| QuarantineFailure::Preparation)?;
    let record = snapshot.installation();
    let provenance = snapshot
        .provenance_verity()
        .ok_or(QuarantineFailure::Preparation)?;
    let policy_id = record
        .key_policy_id
        .as_slice()
        .try_into()
        .map_err(|_| QuarantineFailure::Preparation)?;
    let version =
        u8::try_from(record.signer_key_version).map_err(|_| QuarantineFailure::Preparation)?;
    let fingerprint: String = record
        .signer_fingerprint
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect();
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: the retained descriptor and the correct pointer to the stat structure.
    if unsafe { libc::fstat(inputs.root.directory.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(QuarantineFailure::Preparation);
    }
    let stat = unsafe { stat.assume_init() };
    if stat.st_dev != record.device || stat.st_ino != record.inode {
        return Err(QuarantineFailure::Preparation);
    }
    let candidates = record
        .files
        .iter()
        .map(|file| {
            Ok(RecoveryFileCandidate {
                path: file.path.clone(),
                digest: file
                    .verity_sha256
                    .as_slice()
                    .try_into()
                    .map_err(|_| QuarantineFailure::Preparation)?,
            })
        })
        .collect::<Result<Vec<_>, QuarantineFailure>>()?;
    let root = inputs
        .root
        .directory
        .try_clone()
        .map_err(|_| QuarantineFailure::Preparation)?;
    let files =
        check_recovery_files(root, candidates).map_err(|_| QuarantineFailure::Preparation)?;
    // SAFETY: Single-threaded and trusted root are provided by the caller.
    let mut provenance = unsafe {
        reverify_provenance(ReverifyProvenanceInputs {
            root: inputs.root.directory.as_fd(),
            expected_verity: &provenance,
            signature_policy_path: inputs.signature_policy_path,
            keyring: inputs.keyring,
            verifier: inputs.verifier,
            runtime: inputs.runtime,
            expected_policy_id: policy_id,
            expected_fingerprint: &fingerprint,
            expected_key_version: version,
            timeout: inputs.timeout,
        })
    }
    .map_err(|_| QuarantineFailure::Preparation)?;
    let roles = record
        .files
        .iter()
        .map(|file| {
            let role = match file.role {
                x if x == InstallationFileRole::InstallationPolicy as i32 => {
                    PublicationFileRole::Policy
                }
                x if x == InstallationFileRole::InstallationManifest as i32 => {
                    PublicationFileRole::Manifest
                }
                x if x == InstallationFileRole::InstallationProfile as i32 => {
                    PublicationFileRole::Profile
                }
                x if x == InstallationFileRole::InstallationExecutable as i32 => {
                    PublicationFileRole::Executable
                }
                x if x == InstallationFileRole::InstallationRuntime as i32 => {
                    PublicationFileRole::Runtime
                }
                _ => return Err(QuarantineFailure::Preparation),
            };
            Ok((file.path.clone(), role))
        })
        .collect::<Result<Vec<_>, QuarantineFailure>>()?;
    let contents = reverify_contents(
        &mut provenance,
        &files,
        inputs.supervisor_policy_path,
        inputs.decoder,
        RecoveryExpected {
            package: &record.package_name,
            version: &record.version,
            profile: &record.profile_name,
            profile_path: &record.profile_path,
            executable_path: &record.executable_path,
            files: &roles,
        },
    )
    .map_err(|_| QuarantineFailure::Preparation)?;
    let mut result = ReverifiedQuarantine {
        root: inputs.root.try_clone()?,
        snapshot,
        provenance,
        files,
        contents,
        package_key: package_key.to_vec(),
        installation_key: installation_key.to_vec(),
        invalidated: false,
    };
    result.revalidate(store)?;
    inputs.root.revalidate()?;
    Ok(result)
}
