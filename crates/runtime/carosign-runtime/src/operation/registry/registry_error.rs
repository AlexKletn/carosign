use core::fmt;

/// Registry error.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegistryError {
    /// There's no such operation.
    NotFound,
    /// There are as many operations as are allowed.
    TooMany,
    /// There's already an operation with that number.
    Duplicate,
    /// Secure completion and transfer of resources have not been confirmed.
    CleanupUnverified,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFound => "операция не найдена",
            Self::TooMany => "слишком много операций",
            Self::Duplicate => "операция с таким номером уже есть",
            Self::CleanupUnverified => "безопасное завершение операции не подтверждено",
        })
    }
}

impl core::error::Error for RegistryError {}
