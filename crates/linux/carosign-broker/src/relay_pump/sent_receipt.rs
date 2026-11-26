/// Confirmation of local shipment, not confirmation of employee performance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SentReceipt {
    pub(crate) request_id: [u8; 16],
    pub(crate) is_control: bool,
}
