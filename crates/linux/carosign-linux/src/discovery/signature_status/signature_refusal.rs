/// Refusal to review the report or the absence of an approved signatory.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::discovery) enum SignatureRefusal {
    VerifierFailed,
    InvalidStatus,
    InvalidFingerprint,
    NotApproved,
}
