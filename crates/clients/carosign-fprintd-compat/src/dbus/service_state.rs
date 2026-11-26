use crate::translate::FprintdPhase;
use crate::translate::FprintdState;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ServiceState {
    pub phase: FprintdPhase,
    pub state: FprintdState,
}

impl ServiceState {
    /// Whether the adapter can accept requests.
    ///
    /// Requests require a ready device.
    pub fn ready(self) -> bool {
        matches!(self.phase, FprintdPhase::Idle) && matches!(self.state, FprintdState::Idle)
    }

    pub fn unavailable() -> Self {
        Self {
            phase: FprintdPhase::NotOwned,
            state: FprintdState::Unavailable,
        }
    }
}
