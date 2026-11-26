use core::fmt;

/// How the call ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallResult {
    /// Local policy checks have passed; the handler has not been called yet.
    /// Active issuance and confidence in the source of the parameters are not checked here.
    Handled,
    /// There's no such handler.
    Unsupported,
    /// Not allowed.
    Unauthorized,
    /// Too often.
    RateLimited,
}

impl fmt::Display for CallResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Handled => "локальная проверка пройдена",
            Self::Unsupported => "обработчик не объявлен",
            Self::Unauthorized => "не разрешено",
            Self::RateLimited => "слишком часто",
        })
    }
}
