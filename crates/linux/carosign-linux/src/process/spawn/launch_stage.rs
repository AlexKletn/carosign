/// Where we're in launch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaunchStage {
    /// Nothing yet.
    Idle,
    /// Conditions verified.
    Checked,
    /// The channel is set up.
    Channelled,
    /// A child process has been created, but has not yet executed the program.
    Spawned,
    /// Restrictions and isolation prior to execution are confirmed.
    Isolated,
    /// Successful exec of the fixed object has been confirmed.
    Executed,
    /// The end of the child's canal is closed to the parent.
    Handed,
    /// We received an acceptable greeting from the worker.
    Hello,
    /// Confirmed initialization.
    Initialized,
    /// Confirmed binding.
    Ready,
    /// Refusal.
    Failed,
}
