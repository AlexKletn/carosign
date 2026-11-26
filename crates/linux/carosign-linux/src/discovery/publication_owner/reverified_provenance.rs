use std::os::fd::AsFd;

use crate::discovery::{
    signature_inputs::SignatureCheckedArtifact, signature_policy::SignaturePolicy,
};

use super::{QuarantineRefusal, ReverifyProvenanceInputs, provenance_snapshot::reopen_provenance};

/// The re-verified archive remains linked to the observer of the exact policy that was used
/// for the review, not the launch right or the authorization to quarantine: the installation
/// files and the fresh registry snapshot require separate verification by the owner of the
/// recovery.
pub struct ReverifiedProvenance {
    policy: SignaturePolicy,
    artifact: SignatureCheckedArtifact,
}

impl ReverifiedProvenance {
    pub fn revalidate(&mut self) -> Result<(), QuarantineRefusal> {
        self.policy.unchanged().map_err(|_| QuarantineRefusal)
    }

    pub(super) fn artifact(&self) -> &SignatureCheckedArtifact {
        &self.artifact
    }
}

/// Re-checks the stored archive and signature according to the current independent policy and
/// the assigned verifier.Measurements refer to an accurate quarantine snapshot and are not
/// considered proof of signature per se.
///
/// # Safety
/// The caller must operate in a separate single-threaded assistant and independently confirm
/// the staging root.
pub unsafe fn reverify_provenance(
    inputs: ReverifyProvenanceInputs<'_>,
) -> Result<ReverifiedProvenance, QuarantineRefusal> {
    let ReverifyProvenanceInputs {
        root,
        expected_verity,
        signature_policy_path,
        keyring,
        verifier,
        runtime,
        expected_policy_id,
        expected_fingerprint,
        expected_key_version,
        timeout,
    } = inputs;
    let provenance = reopen_provenance(root, 0, expected_verity).map_err(|_| QuarantineRefusal)?;
    let mut policy = SignaturePolicy::open(signature_policy_path).map_err(|_| QuarantineRefusal)?;
    if policy.policy_id().map_err(|_| QuarantineRefusal)? != expected_policy_id {
        return Err(QuarantineRefusal);
    }
    let prepared = policy
        .prepare(
            provenance.artifact.as_fd(),
            provenance.signature.as_fd(),
            keyring,
            verifier,
            runtime,
        )
        .map_err(|_| QuarantineRefusal)?;
    // SAFETY: The caller supplies a separate single-threaded helper.
    let artifact = unsafe { policy.verify(prepared, timeout) }.map_err(|_| QuarantineRefusal)?;
    if artifact.signature_inputs().0 != expected_fingerprint
        || artifact.primary_key_version() != expected_key_version
        || policy.policy_id().map_err(|_| QuarantineRefusal)? != expected_policy_id
    {
        return Err(QuarantineRefusal);
    }
    Ok(ReverifiedProvenance { policy, artifact })
}
