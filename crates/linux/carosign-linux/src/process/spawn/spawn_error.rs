use super::LaunchStage;
use crate::process::verified_exec::LaunchError;
use core::fmt;

/// Launch failure.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SpawnError {
    /// Conditions didn't go through.
    Launch(LaunchError),
    /// Wrong step.
    WrongStage {
        /// Where we are.
        stage: LaunchStage,
        /// What kind of transition was requested.
        requested: LaunchStage,
    },
    Sandbox(crate::process::sandbox::SandboxError),
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Launch(error) => write!(f, "запуск запрещён: {error}"),
            Self::WrongStage { stage, requested } => {
                write!(f, "переход {stage:?} → {requested:?} запрещён")
            }
            Self::Sandbox(error) => write!(f, "песочница: {error}"),
        }
    }
}

impl core::error::Error for SpawnError {}
