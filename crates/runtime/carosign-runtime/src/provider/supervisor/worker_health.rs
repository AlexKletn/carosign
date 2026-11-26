use core::fmt;

/// What the supervisor knows about the worker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkerHealth {
    /// It didn't start.
    Absent,
    /// It's on, it's shaking hands.
    Starting,
    Ready,
    /// Cleaning up.
    Draining,
    /// Missing.
    Lost,
    /// Rejection: Session closed.
    Faulted,
}

impl fmt::Display for WorkerHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Absent => "отсутствует",
            Self::Starting => "запускается",
            Self::Ready => "готов",
            Self::Draining => "убирается",
            Self::Lost => "пропал",
            Self::Faulted => "отказ",
        })
    }
}
