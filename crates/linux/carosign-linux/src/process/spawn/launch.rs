use super::LaunchStage;
use super::SpawnError;
use crate::process::pidfd::ProcessState;
use crate::process::sandbox::Sandbox;
use crate::process::verified_exec::Preconditions;
use crate::process::verified_exec::check;

/// Launch vehicle.
#[derive(Debug)]
pub struct Launch {
    stage: LaunchStage,
    process: ProcessState,
    failed_at: Option<LaunchStage>,
}

impl Default for Launch {
    fn default() -> Self {
        Self::new()
    }
}

impl Launch {
    /// It's starting.
    pub fn new() -> Self {
        Self {
            stage: LaunchStage::Idle,
            process: ProcessState::Unknown,
            failed_at: None,
        }
    }

    /// Step.
    pub fn stage(&self) -> LaunchStage {
        self.stage
    }

    /// Status of the process.
    pub fn process(&self) -> ProcessState {
        self.process
    }

    /// The step where the rejection occurred.
    pub fn failed_at(&self) -> Option<LaunchStage> {
        self.failed_at
    }

    /// Checking conditions and the sandbox.
    pub fn check(
        &mut self,
        conditions: &Preconditions,
        sandbox: &Sandbox,
        relaxations: &[crate::process::sandbox::Relaxation],
    ) -> Result<(), SpawnError> {
        if self.stage != LaunchStage::Idle {
            return Err(SpawnError::WrongStage {
                stage: self.stage,
                requested: LaunchStage::Checked,
            });
        }
        check(conditions).map_err(SpawnError::Launch)?;
        sandbox.check(relaxations).map_err(SpawnError::Sandbox)?;
        self.stage = LaunchStage::Checked;
        Ok(())
    }

    /// Marks the established channel.
    pub fn channelled(&mut self) -> Result<(), SpawnError> {
        self.advance(LaunchStage::Checked, LaunchStage::Channelled)
    }

    /// Notes the established process; the execution of the program has not yet been
    /// confirmed.
    pub fn spawned(&mut self) -> Result<(), SpawnError> {
        self.advance(LaunchStage::Channelled, LaunchStage::Spawned)?;
        self.process = ProcessState::Running;
        Ok(())
    }

    /// Notes confirmed isolation and limitations.
    pub fn isolated(&mut self) -> Result<(), SpawnError> {
        self.advance(LaunchStage::Spawned, LaunchStage::Isolated)
    }

    /// Marks confirmed exec.
    pub fn executed(&mut self) -> Result<(), SpawnError> {
        self.advance(LaunchStage::Isolated, LaunchStage::Executed)
    }

    /// The channel has been closed and closed.
    pub fn handed(&mut self) -> Result<(), SpawnError> {
        self.advance(LaunchStage::Executed, LaunchStage::Handed)
    }

    /// Celebrates a proven greeting.
    pub fn hello(&mut self) -> Result<(), SpawnError> {
        self.advance(LaunchStage::Handed, LaunchStage::Hello)
    }

    /// Notes confirmed initialization.
    pub fn initialized(&mut self) -> Result<(), SpawnError> {
        self.advance(LaunchStage::Hello, LaunchStage::Initialized)
    }

    /// Marks a confirmed binding.
    pub fn ready(&mut self) -> Result<(), SpawnError> {
        self.advance(LaunchStage::Initialized, LaunchStage::Ready)
    }

    /// Celebrates rejection.
    pub fn fail(&mut self) {
        if self.stage != LaunchStage::Failed {
            self.failed_at = Some(self.stage);
        }
        self.stage = LaunchStage::Failed;
    }

    fn advance(&mut self, from: LaunchStage, to: LaunchStage) -> Result<(), SpawnError> {
        if self.stage != from {
            return Err(SpawnError::WrongStage {
                stage: self.stage,
                requested: to,
            });
        }
        self.stage = to;
        Ok(())
    }
}
