use super::super::gate::PolicyAdmissionError;
use super::super::gate::PolicyAdmissions;
use crate::policy::PolicyChangeWatch;
use crate::policy::PolicyWatchState;

pub(super) struct State {
    pub(super) watch: PolicyChangeWatch,
    pub(super) admissions: PolicyAdmissions,
    pub(super) invalidated: bool,
}

impl State {
    pub(super) fn invalidate(&mut self) -> Result<(), PolicyAdmissionError> {
        if self.invalidated {
            return Err(PolicyAdmissionError::Closed);
        }
        self.invalidated = true;
        self.admissions.revoke()
    }

    pub(super) fn ensure_current(&mut self) -> Result<(), PolicyAdmissionError> {
        if self.invalidated {
            return Err(PolicyAdmissionError::Closed);
        }
        if !matches!(self.watch.check(), Ok(PolicyWatchState::Unchanged)) {
            let _ = self.invalidate();
            return Err(PolicyAdmissionError::Closed);
        }
        Ok(())
    }
}
