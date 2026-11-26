use core::fmt;

/// What you see about the trial.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProcessState {
    /// Alive.
    Running,
    /// It's over.
    Exited {
        /// Code, if known.
        code: Option<i32>,
    },
    /// Killed by the signal.
    Signalled,
    /// Surveillance lost.
    Unknown,
}

impl fmt::Display for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => f.write_str("работает"),
            Self::Exited { code } => write!(f, "завершился ({code:?})"),
            Self::Signalled => f.write_str("убит сигналом"),
            Self::Unknown => f.write_str("состояние неизвестно"),
        }
    }
}
