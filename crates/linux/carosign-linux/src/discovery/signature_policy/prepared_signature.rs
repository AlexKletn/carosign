use super::super::signature_inputs::PinnedVerifier;
use super::super::signature_inputs::SignatureInputs;
use super::super::signature_runtime::PinnedRuntime;
use std::sync::Arc;

pub(in crate::discovery) struct PreparedSignature {
    pub(super) inputs: SignatureInputs,
    pub(super) verifier: PinnedVerifier,
    pub(super) runtime: PinnedRuntime,
    pub(super) primary: String,
    pub(super) policy_identity: Arc<()>,
}
