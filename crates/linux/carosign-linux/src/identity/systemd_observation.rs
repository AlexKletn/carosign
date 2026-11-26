#[derive(Debug, PartialEq, Eq)]
pub(super) struct SystemdObservation {
    pub(super) owner: String,
    pub(super) unit: String,
    pub(super) invocation: [u8; 16],
}
