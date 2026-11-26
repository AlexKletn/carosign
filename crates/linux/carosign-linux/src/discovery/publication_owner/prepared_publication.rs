use carosign_core::ids::{PackageId, ProfileId, ProviderId};
use std::collections::BTreeMap;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::time::Duration;

use crate::discovery::{
    archive,
    signature_inputs::SignatureCheckedArtifact,
    signature_policy::{PreparedSignature, SignaturePolicy},
    supervisor_policy::SupervisorPolicy,
};
use crate::policy::{PolicyChangeWatch, PolicyWatchState, ProtectedPolicySnapshot};

use super::{
    InstallationMetadataDecoder, PreparationError, PublicationRecordFields, StagingLocation,
    directory_metadata::directory_metadata,
    initial_identities::{InitialIdentities, allocate_initial_identities},
    provenance_snapshot::{PROVENANCE_ARTIFACT, PROVENANCE_SIGNATURE},
    publication_file_role::publication_file_role,
    stage_policy::{policy_matches_snapshot, stage_policy},
    stage_provenance::stage_provenance,
};

// This name belongs to the installer, not to the signed archive, and its matching with the
// name in the archive is forbidden: the policy and file of the package have different
// origins.
pub(in crate::discovery) const STAGED_POLICY_PATH: &str = ".carosign-supervisor-policy.toml";
/// Holds the signed photo, both policies and all measured files.
/// The mere existence of this object does not permit either commit or exec.
pub(super) struct PreparedPublication {
    signature_policy: SignaturePolicy,
    supervisor_watch: PolicyChangeWatch,
    supervisor_policy: SupervisorPolicy,
    artifact: SignatureCheckedArtifact,
    // Hold the attached images until the quarantine commit is completed.
    _provenance: Vec<(OwnedFd, [u8; 32])>,
    staging_root: OwnedFd,
    files: BTreeMap<String, (OwnedFd, [u8; 32])>,
    /// The path is derived from a signed manifesto already verified, not from one that causes
    /// future publication.
    profile_path: String,
    identities: InitialIdentities,
}

/// Re-articulates the retained immutable manifesto for the future record holder, and the path
/// and verification are not permission to publish.
fn retained_profile_path(
    files: &BTreeMap<String, (OwnedFd, [u8; 32])>,
    policy: &SupervisorPolicy,
    decoder: &dyn InstallationMetadataDecoder,
) -> Result<String, PreparationError> {
    let (fd, _) = files
        .get("package.toml")
        .ok_or(PreparationError::InvalidSupervisorPolicy)?;
    let file = std::fs::File::from(fd.try_clone().map_err(PreparationError::Io)?);
    let len = file.metadata().map_err(PreparationError::Io)?.len();
    if len > 65_536 {
        return Err(PreparationError::InvalidSupervisorPolicy);
    }
    let mut bytes = vec![0; len as usize];
    file.read_exact_at(&mut bytes, 0)
        .map_err(PreparationError::Io)?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| PreparationError::InvalidSupervisorPolicy)?;
    let manifest = decoder
        .manifest(text, &policy.profile)
        .map_err(|_| PreparationError::InvalidSupervisorPolicy)?;
    if manifest.package != policy.package
        || manifest.version != policy.version
        || manifest.executable != policy.executable
        || !files.contains_key(&manifest.profile_path)
    {
        return Err(PreparationError::InvalidSupervisorPolicy);
    }
    Ok(manifest.profile_path)
}

impl PreparedPublication {
    /// Extracts fields from one withheld training and reconfirms policies and the staging
    /// association with the name.Appraisal does not serialize a review followed by commit;
    /// entry must remain in quarantine.
    pub(super) fn record_fields(
        &mut self,
        location: &StagingLocation,
    ) -> Result<PublicationRecordFields, PreparationError> {
        self.revalidate_policies()?;
        location.revalidate().map_err(PreparationError::Io)?;
        let actual = directory_metadata(self.staging_root.as_fd(), location.owner)
            .map_err(PreparationError::Io)?;
        let bound = directory_metadata(location.staging.as_fd(), location.owner)
            .map_err(PreparationError::Io)?;
        if actual.st_dev != bound.st_dev || actual.st_ino != bound.st_ino {
            return Err(PreparationError::InvalidSupervisorPolicy);
        }
        let policy_id = self.key_policy_id()?;
        let (signer, _) = self.artifact.signature_inputs();
        let files = self
            .files
            .iter()
            .map(|(path, (_, digest))| {
                let role = publication_file_role(
                    path,
                    &self.profile_path,
                    &self.supervisor_policy.executable,
                    &self.supervisor_policy.runtime,
                )
                .ok_or(PreparationError::InvalidSupervisorPolicy)?;
                Ok((path.clone(), role, *digest))
            })
            .collect::<Result<Vec<_>, PreparationError>>()?;
        let result = PublicationRecordFields {
            package: self.identities.package,
            provider: self.identities.provider,
            profile: self.identities.profile,
            policy_id,
            package_name: self.supervisor_policy.package.clone(),
            version: self.supervisor_policy.version.clone(),
            profile_name: self.supervisor_policy.profile.clone(),
            version_dir: location.version_dir.clone(),
            device: actual.st_dev,
            inode: actual.st_ino,
            signer_version: self.artifact.primary_key_version(),
            signer_fingerprint: signer.to_owned(),
            profile_path: self.profile_path.clone(),
            executable_path: self.supervisor_policy.executable.clone(),
            files,
            provenance_verity: [self._provenance[0].1, self._provenance[1].1],
        };
        location.revalidate().map_err(PreparationError::Io)?;
        self.revalidate_policies()?;
        Ok(result)
    }

    /// The link of the name to staging is checked against the retained root.
    /// The binding itself does not give permission to publish or launch.
    pub(super) fn bind_location(
        &mut self,
        installation_root: BorrowedFd<'_>,
        version_dir: &str,
        owner: u32,
    ) -> Result<StagingLocation, PreparationError> {
        self.revalidate_policies()?;
        let location = StagingLocation::bind(
            installation_root,
            self.staging_root.as_fd(),
            version_dir,
            owner,
        )
        .map_err(PreparationError::Io)?;
        self.revalidate_policies()?;
        Ok(location)
    }

    /// The ID of the independent signature policy is only obtained from the image that's been
    /// viewed, which has been used to test this artifact.
    fn key_policy_id(&mut self) -> Result<[u8; 16], PreparationError> {
        self.revalidate_policies()?;
        let id = self
            .signature_policy
            .policy_id()
            .map_err(PreparationError::SignaturePolicy)?;
        self.revalidate_policies()?;
        Ok(id)
    }

    /// Platform-assigned identities are held by the same owner as the signed artifact and
    /// observable policies, until the decision to record.
    fn initial_identities(
        &mut self,
    ) -> Result<(PackageId, ProviderId, ProfileId), PreparationError> {
        self.revalidate_policies()?;
        Ok((
            self.identities.package,
            self.identities.provider,
            self.identities.profile,
        ))
    }

    /// Re-checking images does not serialize a review with a future commit.
    pub(super) fn revalidate_policies(&mut self) -> Result<(), PreparationError> {
        self.signature_policy
            .unchanged()
            .map_err(PreparationError::SignaturePolicy)?;
        check_watch(&mut self.supervisor_watch)?;
        Ok(())
    }
}

fn check_watch(watch: &mut PolicyChangeWatch) -> Result<(), PreparationError> {
    match watch.check().map_err(PreparationError::Io)? {
        PolicyWatchState::Unchanged => Ok(()),
        PolicyWatchState::Invalidated => Err(PreparationError::PolicyChanged),
    }
}

/// Verifies the signature and composition, then holds both policies until published.
/// In case of an error, the caller destroys the partially prepared closed directory.
///
/// # Safety
/// The caller must be a separate single-threaded assistant, as required by the isolated
/// signature checker, and not pass someone else's staging directory.
pub(super) unsafe fn prepare_publication(
    mut signature_policy: SignaturePolicy,
    prepared_signature: PreparedSignature,
    supervisor_policy_path: &Path,
    decoder: &dyn InstallationMetadataDecoder,
    staging_root: BorrowedFd<'_>,
    timeout: Duration,
) -> Result<PreparedPublication, PreparationError> {
    let (contents, mut supervisor_watch) = ProtectedPolicySnapshot::open(supervisor_policy_path)
        .map_err(PreparationError::Io)?
        .into_parts();
    let supervisor_policy = SupervisorPolicy::parse(&contents)
        .map_err(|_| PreparationError::InvalidSupervisorPolicy)?;
    check_watch(&mut supervisor_watch)?;
    // The root is held together with the files; the directory name is not its ID.
    let root_fd = unsafe { libc::fcntl(staging_root.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if root_fd < 0 {
        return Err(PreparationError::Io(io::Error::last_os_error()));
    }
    let staging_root = unsafe { OwnedFd::from_raw_fd(root_fd) };
    let artifact = unsafe { signature_policy.verify(prepared_signature, timeout) }
        .map_err(PreparationError::SignaturePolicy)?;
    check_watch(&mut supervisor_watch)?;
    let mut files = archive::inspect_policy_and_stage_measured(
        &artifact,
        decoder,
        &supervisor_policy,
        std::os::fd::AsFd::as_fd(&staging_root),
    )
    .map_err(PreparationError::Archive)?;
    // The images themselves are not added to the runtime role: they are only needed to
    // independently re-check the signature, not to run the worker.
    // O_EXCL It prevents the archive from taking reserved names.
    let provenance = [
        (PROVENANCE_ARTIFACT, artifact.as_fd(), 256 * 1024 * 1024),
        (PROVENANCE_SIGNATURE, artifact.signature_fd(), 1024 * 1024),
    ]
    .into_iter()
    .map(|(name, source, max)| {
        stage_provenance(staging_root.as_fd(), name, source, max, |fd| {
            crate::discovery::verity::enable_and_measure(fd)
        })
        .map_err(PreparationError::Io)
    })
    .collect::<Result<Vec<_>, _>>()?;
    let profile_path = retained_profile_path(&files, &supervisor_policy, decoder)?;
    // Politics is not taken from an archive, but from an independent, protected image.
    // The mismatch of origin cannot be concealed by the common file name.
    if files.contains_key(STAGED_POLICY_PATH) {
        return Err(PreparationError::InvalidSupervisorPolicy);
    }
    let policy_file =
        stage_policy(staging_root.as_fd(), &contents).map_err(PreparationError::Io)?;
    let digest = crate::discovery::verity::enable_and_measure(policy_file.as_fd())
        .map_err(PreparationError::Io)?;
    if !policy_matches_snapshot(&policy_file, &contents).map_err(PreparationError::Io)? {
        return Err(PreparationError::PolicyChanged);
    }
    if unsafe { libc::fsync(policy_file.as_raw_fd()) } != 0 {
        return Err(PreparationError::Io(io::Error::last_os_error()));
    }
    files.insert(STAGED_POLICY_PATH.to_owned(), (policy_file, digest));
    // Appointment occurs only after verification of the signature, composition and files.
    // This path is suitable only for the first installation: the re-installation must obtain
    // the existing comparison from the verified registry.
    let identities = allocate_initial_identities(|| crate::ids::KernelIds::new().bytes().ok())?;
    let mut result = PreparedPublication {
        signature_policy,
        supervisor_watch,
        supervisor_policy,
        artifact,
        _provenance: provenance,
        staging_root,
        files,
        profile_path,
        identities,
    };
    result.revalidate_policies()?;
    Ok(result)
}

#[cfg(test)]
#[path = "prepared_publication.tests.rs"]
mod tests;
