use super::WorkerHealth;
use core::fmt;

/// Supervisor's refusal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SupervisorError {
    /// It's already on.
    AlreadyRunning,
    /// No one's running.
    NotRunning,
    /// The previous era is outdated by a relatively well-known supervisor.
    StaleEpoch,
    /// Epoch counter is exhausted; the first one cannot be reused.
    EpochExhausted,
    /// Wrong stage.
    WrongPhase {
        expected: WorkerHealth,
        /// Got it.
        actual: WorkerHealth,
    },
}

impl fmt::Display for SupervisorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => f.write_str("работник уже запущен"),
            Self::NotRunning => f.write_str("работник не запущен"),
            Self::StaleEpoch => f.write_str("предыдущая эпоха устарела"),
            Self::EpochExhausted => f.write_str("счётчик эпох исчерпан"),
            Self::WrongPhase { expected, actual } => {
                write!(f, "стадия {actual}, а нужна {expected}")
            }
        }
    }
}

impl core::error::Error for SupervisorError {}
