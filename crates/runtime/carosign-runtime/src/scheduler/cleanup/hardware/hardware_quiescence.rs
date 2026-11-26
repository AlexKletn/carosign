use super::super::CleanupPortFailure;
use super::super::CleanupScope;
use super::super::QuiescedLease;
use super::Observation;
use super::Stage;

/// Compars trusted observations with one withheld issue.
/// Without the implementation of a hardware source, this model does not establish peace.
pub struct HardwareQuiescence {
    scope: CleanupScope,
    stage: Stage,
}

fn may_advance(current: Stage, next: Stage) -> bool {
    matches!(
        (current, next),
        (Stage::Bound, Stage::AdmissionsClosed)
            | (Stage::AdmissionsClosed, Stage::InterruptsDrained)
            | (Stage::InterruptsDrained, Stage::CallsDrained)
            | (Stage::CallsDrained, Stage::DeviceQuiesced)
    )
}

impl HardwareQuiescence {
    pub fn new(scope: CleanupScope) -> Self {
        Self {
            scope,
            stage: Stage::Bound,
        }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Alien issuance, repetition and jump through the step are rejected without change.
    pub fn advance(&mut self, observation: Observation) -> Result<(), Observation> {
        if observation.scope != self.scope || !may_advance(self.stage, observation.reached) {
            return Err(observation);
        }
        self.stage = observation.reached;
        Ok(())
    }

    /// Forms a one-time confirmation only for the same fence.
    /// It must be accepted by a trusted broker in the `confirm_released`; this call
    /// does not change either the delivery or the stable storage.
    pub fn confirm_for(
        self,
        held: &super::super::super::Quiescing,
    ) -> Result<QuiescedLease, CleanupPortFailure> {
        if self.stage != Stage::DeviceQuiesced
            || held.lease != self.scope.lease_id()
            || held.owner != self.scope.owner()
            || &held.set != self.scope.resource_set()
        {
            return Err(CleanupPortFailure::Uncertain);
        }
        Ok(QuiescedLease {
            scope: self.scope,
            fence: Some(held.key.clone()),
        })
    }
}

#[path = "hardware_quiescence.tests.rs"]
#[cfg(test)]
mod tests;
