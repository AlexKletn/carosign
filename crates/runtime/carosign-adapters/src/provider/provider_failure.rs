use core::fmt;

/// Wire failure.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProviderFailure {
    /// Canal's closed.
    Closed,
    /// Footage not accepted.
    Rejected,
    /// The shipment failed.
    SendFailed,
}

impl fmt::Display for ProviderFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Closed => "канал провайдера закрыт",
            Self::Rejected => "кадр отвергнут",
            Self::SendFailed => "отправка не удалась",
        })
    }
}

impl core::error::Error for ProviderFailure {}
