use core::fmt;

/// The context is wrong.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContextError {
    /// Incorrect initialization message or instance identifier
    MissingInstance,
    /// The binding or its reference to the device is incorrect.
    MissingAuthenticator,
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingInstance => "неверная инициализация работника",
            Self::MissingAuthenticator => "неверная привязка аппарата",
        })
    }
}

impl core::error::Error for ContextError {}
