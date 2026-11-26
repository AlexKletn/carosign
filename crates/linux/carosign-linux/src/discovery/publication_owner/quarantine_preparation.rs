use crate::discovery::signature_policy::SignaturePolicy;

use super::{
    PreparedPublication, PublicationRecordFields, QuarantineInputs, QuarantineRefusal,
    StagingLocation, prepare_publication,
};

/// Owns verified files and surveillance policies; does not allow launch.
pub struct QuarantinePreparation {
    prepared: PreparedPublication,
    location: StagingLocation,
}

impl QuarantinePreparation {
    /// A snapshot of the data is not an authority; fixation is permissible only in
    /// quarantine.
    pub fn record_fields(&mut self) -> Result<PublicationRecordFields, QuarantineRefusal> {
        self.prepared
            .record_fields(&self.location)
            .map_err(|_| QuarantineRefusal)
    }

    /// The check is not serialized with fixation and does not lift quarantine.
    pub fn revalidate(&mut self) -> Result<(), QuarantineRefusal> {
        self.prepared
            .revalidate_policies()
            .map_err(|_| QuarantineRefusal)?;
        self.location.revalidate().map_err(|_| QuarantineRefusal)?;
        self.prepared
            .revalidate_policies()
            .map_err(|_| QuarantineRefusal)
    }
}

/// Checks the signed package and holds it until it is quarantined.
///
/// # Safety
/// Calling is only acceptable in a single-threaded assistant; the root of the installation
/// and staging must be independently verified by the system owner.
pub unsafe fn prepare_quarantine(
    inputs: QuarantineInputs<'_>,
) -> Result<QuarantinePreparation, QuarantineRefusal> {
    let QuarantineInputs {
        signature_policy_path,
        supervisor_policy_path,
        artifact,
        signature,
        keyring,
        verifier,
        runtime,
        decoder,
        installation_root,
        staging,
        version_dir,
        timeout,
    } = inputs;
    let mut policy = SignaturePolicy::open(signature_policy_path).map_err(|_| QuarantineRefusal)?;
    let inputs = policy
        .prepare(artifact, signature, keyring, verifier, runtime)
        .map_err(|_| QuarantineRefusal)?;
    let mut prepared = unsafe {
        prepare_publication(
            policy,
            inputs,
            supervisor_policy_path,
            decoder,
            staging,
            timeout,
        )
    }
    .map_err(|_| QuarantineRefusal)?;
    let location = prepared
        .bind_location(installation_root, version_dir, 0)
        .map_err(|_| QuarantineRefusal)?;
    Ok(QuarantinePreparation { prepared, location })
}
