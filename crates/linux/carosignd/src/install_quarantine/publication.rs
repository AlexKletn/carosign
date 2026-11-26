use super::{
    InstallationFile, InstallationFileRole, InstallationState, PackageRecord,
    ProviderInstallationRecord, PublicationFileRole, PublicationRecordFields, QuarantineFailure,
    QuarantineInputs, QuarantinePreparation, RedbStore, RegisteredProfile, decode_fingerprint,
    prepare_quarantine,
};

/// Preparation and quarantine fixation in one process retain FD possession.
///
/// # Safety
/// The caller is required to work in a separate single-threaded assistant and independently
/// confirm the root of the installation and staging according to the Linux adapter contract.
/// Successful preparation does not remove quarantine and does not allow launch.
pub(crate) unsafe fn prepare_and_commit(
    inputs: QuarantineInputs<'_>,
    store: &RedbStore,
) -> Result<(), QuarantineFailure> {
    // SAFETY: The conditions of the individual helper and roots provide the caller.
    let preparation =
        unsafe { prepare_quarantine(inputs) }.map_err(|_| QuarantineFailure::Preparation)?;
    commit(preparation, store)
}

pub(super) fn records(
    fields: PublicationRecordFields,
) -> Result<(PackageRecord, ProviderInstallationRecord), QuarantineFailure> {
    let package_id = fields.package.as_bytes().to_vec();
    let provider_id = fields.provider.as_bytes().to_vec();
    let profile_id = fields.profile.as_bytes().to_vec();
    let package = PackageRecord {
        package_name: fields.package_name.clone(),
        package_id: package_id.clone(),
        provider_id: provider_id.clone(),
        installed_version: fields.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: fields.profile_name.clone(),
            profile_id: profile_id.clone(),
        }],
    };
    let files = fields
        .files
        .into_iter()
        .map(|(path, role, digest)| InstallationFile {
            path,
            role: match role {
                PublicationFileRole::Policy => InstallationFileRole::InstallationPolicy,
                PublicationFileRole::Manifest => InstallationFileRole::InstallationManifest,
                PublicationFileRole::Profile => InstallationFileRole::InstallationProfile,
                PublicationFileRole::Executable => InstallationFileRole::InstallationExecutable,
                PublicationFileRole::Runtime => InstallationFileRole::InstallationRuntime,
            } as i32,
            verity_sha256: digest.to_vec(),
        })
        .collect();
    let installation = ProviderInstallationRecord {
        package_id,
        provider_id,
        profile_id,
        package_name: fields.package_name,
        version: fields.version,
        profile_name: fields.profile_name,
        version_dir: fields.version_dir,
        device: fields.device,
        inode: fields.inode,
        signer_key_version: fields.signer_version.into(),
        signer_fingerprint: decode_fingerprint(&fields.signer_fingerprint)?,
        key_policy_id: fields.policy_id.to_vec(),
        revision: 1,
        state: InstallationState::InstallationActive as i32,
        policy_path: ".carosign-supervisor-policy.toml".to_owned(),
        manifest_path: "package.toml".to_owned(),
        profile_path: fields.profile_path,
        executable_path: fields.executable_path,
        files,
    };
    Ok((package, installation))
}

/// The owner is held until the return commit; the unknown outcome is not repeated.
pub(crate) fn commit(
    mut preparation: QuarantinePreparation,
    store: &RedbStore,
) -> Result<(), QuarantineFailure> {
    let fields = preparation
        .record_fields()
        .map_err(|_| QuarantineFailure::Preparation)?;
    let provenance = fields.provenance_verity;
    let (package, installation) = records(fields)?;
    store
        .quarantine_initial_installation_with_provenance(
            &package,
            &installation,
            provenance,
            || preparation.revalidate().is_ok(),
        )
        .map_err(QuarantineFailure::Storage)
}
