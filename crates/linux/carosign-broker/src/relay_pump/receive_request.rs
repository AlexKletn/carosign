#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReceiveRequest {
    Local {
        request_id: [u8; 16],
    },
    Queued {
        request_id: [u8; 16],
    },
    /// The command is not set or started; the owner must respond by BUSY.
    Busy {
        request_id: [u8; 16],
    },
}
