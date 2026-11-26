use super::GateSender;

pub(super) enum AdmissionState {
    Pending { generation: u64, sender: GateSender },
    Released,
    Revoked,
    Failed,
}
