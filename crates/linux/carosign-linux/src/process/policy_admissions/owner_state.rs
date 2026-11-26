use super::WatchedPolicyAdmissions;

pub(super) struct OwnerState {
    pub(super) generation: u64,
    pub(super) active: Option<WatchedPolicyAdmissions>,
}
