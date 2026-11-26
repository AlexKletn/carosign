use super::{SupervisorError, WorkerHealth, WorkerSlot};
use crate::ports::QuiesceAssessment;
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::WorkerInstanceId;

/// Supervisor.
#[derive(Debug, Default)]
pub struct Supervisor {
    slot: Option<WorkerSlot>,
}

impl Supervisor {
    /// Empty supervisor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starting launch.
    ///
    /// ⚠️ The new worker gets a new era: the messages of the old process are no longer valid,
    /// even if he is alive.
    pub fn start(
        &mut self,
        instance: WorkerInstanceId,
        previous_epoch: DeviceEpoch,
    ) -> Result<DeviceEpoch, SupervisorError> {
        if matches!(
            self.slot.as_ref().map(|slot| slot.health),
            Some(WorkerHealth::Ready | WorkerHealth::Starting)
        ) {
            return Err(SupervisorError::AlreadyRunning);
        }
        if self
            .slot
            .as_ref()
            .is_some_and(|slot| previous_epoch < slot.epoch)
        {
            return Err(SupervisorError::StaleEpoch);
        }
        let epoch = previous_epoch
            .next()
            .map_err(|_| SupervisorError::EpochExhausted)?;
        self.slot = Some(WorkerSlot {
            instance,
            epoch,
            health: WorkerHealth::Starting,
        });
        Ok(epoch)
    }

    /// Notes readiness (fact, not warranty).
    pub fn mark_ready(&mut self) -> Result<(), SupervisorError> {
        self.transition(WorkerHealth::Starting, WorkerHealth::Ready)
    }

    /// Marks the loss.
    ///
    /// ⚠️ Returns the cleaning assessment: after the process is lost, it `Uncertain`until the
    /// mediator confirms otherwise.
    pub fn lost(&mut self) -> Result<(DeviceEpoch, QuiesceAssessment), SupervisorError> {
        let epoch = self.current_epoch().ok_or(SupervisorError::NotRunning)?;
        self.set_health(WorkerHealth::Lost);
        Ok((epoch, QuiesceAssessment::Uncertain))
    }

    /// Finishing the cleaning.
    pub fn quarantined(&mut self) -> Result<(), SupervisorError> {
        let slot = self.slot.as_mut().ok_or(SupervisorError::NotRunning)?;
        slot.health = WorkerHealth::Faulted;
        Ok(())
    }

    pub fn current_epoch(&self) -> Option<DeviceEpoch> {
        self.slot.as_ref().map(|slot| slot.epoch)
    }

    /// Current launch.
    pub fn current(&self) -> Option<&WorkerSlot> {
        self.slot.as_ref()
    }

    pub fn is_ready(&self) -> bool {
        matches!(
            self.slot.as_ref().map(|slot| slot.health),
            Some(WorkerHealth::Ready)
        )
    }

    fn transition(&mut self, from: WorkerHealth, to: WorkerHealth) -> Result<(), SupervisorError> {
        let slot = self.slot.as_mut().ok_or(SupervisorError::NotRunning)?;
        if slot.health != from {
            return Err(SupervisorError::WrongPhase {
                expected: from,
                actual: slot.health,
            });
        }
        slot.health = to;
        Ok(())
    }

    fn set_health(&mut self, health: WorkerHealth) {
        if let Some(slot) = self.slot.as_mut() {
            slot.health = health;
        }
    }
}

#[path = "supervisor.tests.rs"]
#[cfg(test)]
mod tests;
