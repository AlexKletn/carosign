/// Information verified by the system installer regardless of TOML.
pub(super) struct ApprovedPolicy {
    pub(super) package: String,
    pub(super) version: String,
    pub(super) profile: String,
    pub(super) executable: String,
    pub(super) executable_verity: [u8; 32],
}
