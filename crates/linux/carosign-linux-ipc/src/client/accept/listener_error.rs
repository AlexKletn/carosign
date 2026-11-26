/// Listener's refusal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListenerError {
    /// The path is not absolute.
    BadPath,
    /// Remote access is not supported.
    RemoteNotSupported,
    AlreadyPublished,
}

impl core::fmt::Display for ListenerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::BadPath => "путь должен быть абсолютным",
            Self::RemoteNotSupported => "удалённый доступ не поддерживается в v1",
            Self::AlreadyPublished => "слушатель уже опубликован",
        })
    }
}

impl core::error::Error for ListenerError {}
