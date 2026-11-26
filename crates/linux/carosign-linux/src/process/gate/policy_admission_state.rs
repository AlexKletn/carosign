use super::GateSender;

pub(super) struct PolicyAdmissionState {
    pub(super) generation: u64,
    pub(super) revoked: bool,
    pub(super) next: u64,
    pub(super) pending: std::collections::BTreeMap<u64, GateSender>,
}
