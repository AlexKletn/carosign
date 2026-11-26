#[derive(Default)]
pub(crate) struct CapabilityState {
    pub(crate) descriptor: Option<carosign_protocol::carosign::common::v1::CapabilityDescriptor>,
    pub(crate) verified_ready: bool,
}
