/// An unverified list from the quarantine record, which is not an authority.
pub struct RecoveryFileCandidate {
    pub path: String,
    pub digest: [u8; 32],
}
