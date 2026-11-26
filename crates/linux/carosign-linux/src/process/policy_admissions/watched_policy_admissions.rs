use super::super::gate::GateSender;
use super::super::gate::PolicyAdmissionError;
use super::super::gate::PolicyAdmissions;
use super::State;
use crate::policy::PolicyChangeWatch;
use crate::policy::PolicyWatchState;
use std::sync::Mutex;

pub(crate) struct WatchedPolicyAdmissions {
    inner: Mutex<State>,
}

impl WatchedPolicyAdmissions {
    pub(crate) fn new(
        generation: u64,
        mut watch: PolicyChangeWatch,
    ) -> Result<Self, PolicyAdmissionError> {
        if !matches!(watch.check(), Ok(PolicyWatchState::Unchanged)) {
            return Err(PolicyAdmissionError::Closed);
        }
        Ok(Self {
            inner: Mutex::new(State {
                watch,
                admissions: PolicyAdmissions::new(generation)?,
                invalidated: false,
            }),
        })
    }

    pub(crate) fn register(
        &self,
        generation: u64,
        sender: GateSender,
    ) -> Result<u64, PolicyAdmissionError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?;
        state.ensure_current()?;
        state.admissions.register(generation, sender)
    }

    pub(crate) fn admit(&self, generation: u64, id: u64) -> Result<(), PolicyAdmissionError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?;
        state.ensure_current()?;
        state.admissions.admit(generation, id)
    }

    pub(crate) fn refresh(&self) -> Result<(), PolicyAdmissionError> {
        self.inner
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?
            .ensure_current()
    }

    pub(crate) fn revoke(&self) -> Result<(), PolicyAdmissionError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?;
        state.invalidate()
    }
}
