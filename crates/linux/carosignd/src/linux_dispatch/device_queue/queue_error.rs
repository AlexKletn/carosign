use carosign_protocol::carosign::common::v1::ErrorCode;
use carosign_runtime::ports::AuthorityRefusal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueueError {
    Limit,
    Timeout,
    Disconnected,
    Authority(AuthorityRefusal),
    Unavailable,
}

impl QueueError {
    pub(crate) fn response(self) -> (ErrorCode, &'static str) {
        match self {
            Self::Limit => (ErrorCode::ErrorBusy, "authenticator queue full"),
            Self::Timeout | Self::Authority(AuthorityRefusal::Expired) => {
                (ErrorCode::ErrorTimeout, "authenticator queue deadline")
            }
            Self::Authority(AuthorityRefusal::Denied) => {
                (ErrorCode::ErrorUnauthorized, "operation unauthorized")
            }
            Self::Authority(AuthorityRefusal::Revoked) => {
                (ErrorCode::ErrorGrantRevoked, "operation grant revoked")
            }
            _ => (
                ErrorCode::ErrorUnavailable,
                "authenticator queue unavailable",
            ),
        }
    }
}
