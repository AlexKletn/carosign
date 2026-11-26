use super::super::signature_inputs::PinnedVerifier;
use super::super::signature_inputs::SignatureCheckedArtifact;
use super::super::signature_inputs::SignatureInputs;
use super::super::signature_runtime::PinnedRuntime;
use super::super::signature_runtime::RuntimeFile;
use super::PolicyError;
use super::PreparedSignature;
use super::Rules;
use crate::policy::PolicyChangeWatch;
use crate::policy::PolicyWatchState;
use crate::policy::ProtectedPolicySnapshot;
use std::collections::BTreeSet;
use std::os::fd::BorrowedFd;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// A snapshot of an independent root-owned policy is held by a review observer.
pub(in crate::discovery) struct SignaturePolicy {
    rules: Rules,
    watch: PolicyChangeWatch,
    identity: Arc<()>,
}

impl SignaturePolicy {
    pub(in crate::discovery) fn open(path: &Path) -> Result<Self, PolicyError> {
        let (bytes, mut watch) = ProtectedPolicySnapshot::open(path)
            .map_err(PolicyError::Io)?
            .into_parts();
        let rules = Rules::parse(&bytes)?;
        if watch.check().map_err(PolicyError::Io)? != PolicyWatchState::Unchanged {
            return Err(PolicyError::Changed);
        }
        Ok(Self {
            rules,
            watch,
            identity: Arc::new(()),
        })
    }

    pub(in crate::discovery) fn unchanged(&mut self) -> Result<(), PolicyError> {
        if self.watch.check().map_err(PolicyError::Io)? != PolicyWatchState::Unchanged {
            return Err(PolicyError::Changed);
        }
        Ok(())
    }

    /// The identifier is taken from the same image that is observed as the full set of keys
    /// and measurements; it is not an authority in itself.
    pub(in crate::discovery) fn policy_id(&mut self) -> Result<[u8; 16], PolicyError> {
        self.unchanged()?;
        Ok(self.rules.policy_id)
    }

    /// Verifies the exact set of retained files with a separate pre- and post-preparation
    /// policy, the result is not permission to publish or run.
    pub(in crate::discovery) fn prepare<'a>(
        &mut self,
        artifact: BorrowedFd<'a>,
        signature: BorrowedFd<'a>,
        keyring: BorrowedFd<'a>,
        verifier: BorrowedFd<'a>,
        runtime: &[(&str, BorrowedFd<'a>)],
    ) -> Result<PreparedSignature, PolicyError> {
        self.unchanged()?;
        let names: BTreeSet<&str> = runtime.iter().map(|(name, _)| *name).collect();
        if names.len() != runtime.len()
            || names.len() != self.rules.runtime.len()
            || !names
                .iter()
                .all(|name| self.rules.runtime.contains_key(*name))
        {
            return Err(PolicyError::Invalid);
        }
        let files: Vec<_> = runtime
            .iter()
            .map(|(name, source)| RuntimeFile {
                name,
                source: *source,
                approved_verity: self.rules.runtime[*name],
            })
            .collect();
        let pinned_runtime = PinnedRuntime::prepare(&files).map_err(PolicyError::Runtime)?;
        let pinned_verifier =
            PinnedVerifier::prepare(verifier, &self.rules.verifier).map_err(PolicyError::Input)?;
        let inputs = SignatureInputs::prepare(artifact, signature, keyring, &self.rules.keyring)
            .map_err(PolicyError::Input)?;
        self.unchanged()?;
        Ok(PreparedSignature {
            inputs,
            verifier: pinned_verifier,
            runtime: pinned_runtime,
            primary: self.rules.primary.clone(),
            policy_identity: Arc::clone(&self.identity),
        })
    }

    /// The verifier is called in a separate single-threaded assistant; after its return, the
    /// policy recall is checked again before transferring the image further.
    ///
    /// # Safety
    /// The caller must provide a separate single-threaded process-assistant.
    pub(in crate::discovery) unsafe fn verify(
        &mut self,
        prepared: PreparedSignature,
        timeout: Duration,
    ) -> Result<SignatureCheckedArtifact, PolicyError> {
        // Preparations are tied to the observer who authorized its entrances: another
        // politician cannot confirm someone else's picture.
        if !Arc::ptr_eq(&self.identity, &prepared.policy_identity) {
            return Err(PolicyError::Invalid);
        }
        self.unchanged()?;
        let checked = unsafe {
            prepared.inputs.verify_in_single_threaded_helper(
                &prepared.verifier,
                &prepared.runtime,
                &prepared.primary,
                timeout,
            )
        }
        .map_err(PolicyError::Input)?;
        self.unchanged()?;
        Ok(checked)
    }
}
