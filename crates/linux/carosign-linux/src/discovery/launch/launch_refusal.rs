use crate::process::verified_exec::LaunchError;
use core::fmt;

/// How the preparations ended.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LaunchRefusal {
    /// Conditions didn't go through.
    Conditions(LaunchError),
    /// The kernel failed when opened or reconciled.
    Kernel(i32),
    /// The file was swapped between checking and opening.
    Replaced,
    /// System confirmation of installation is not available.
    InstallationUnavailable,
    /// The installation failed to pass the test.
    InstallationRejected,
}

impl fmt::Display for LaunchRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conditions(error) => write!(f, "условия запуска: {error}"),
            Self::Kernel(errno) => write!(f, "ядро отказало: {errno}"),
            Self::Replaced => f.write_str("файл подменили между проверкой и открытием"),
            Self::InstallationUnavailable => f.write_str("подтверждение установки недоступно"),
            Self::InstallationRejected => f.write_str("установка не прошла проверку"),
        }
    }
}

impl core::error::Error for LaunchRefusal {}
