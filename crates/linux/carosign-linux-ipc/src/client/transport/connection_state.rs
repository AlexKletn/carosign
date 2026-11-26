use carosign_protocol::carosign::client::v1 as wire;

pub(super) struct ConnectionState {
    pub(super) closed: bool,
    pub(super) events: std::collections::VecDeque<wire::ClientFrame>,
    pub(super) pending_request: Option<[u8; 16]>,
    pub(super) response: Option<wire::ClientFrame>,
}
