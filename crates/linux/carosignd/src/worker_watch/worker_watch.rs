use super::{
    DeviceEpoch, GrantResult, LeaseId, Mutex, PidFd, Request, Scheduler, SysError, WatchStage,
    WorkerInstanceId,
};

pub(crate) struct WorkerWatch {
    pidfd: PidFd,
    worker: WorkerInstanceId,
    epoch: DeviceEpoch,
    stage: WatchStage,
}

impl WorkerWatch {
    /// The caller must keep the confirmed binding of the process to the worker.
    pub(crate) fn new(pidfd: PidFd, worker: WorkerInstanceId, epoch: DeviceEpoch) -> Self {
        Self {
            pidfd,
            worker,
            epoch,
            stage: WatchStage::Watching,
        }
    }

    /// In case of exit or error, the surveillance closes the relevant issuances.
    /// A mistake does not prove death, but it does not allow you to continue working.
    pub(crate) fn poll(&mut self, scheduler: &Mutex<Scheduler>) -> Result<WatchStage, SysError> {
        if let WatchStage::Fenced { .. } = self.stage {
            return Ok(self.stage);
        }
        let observation = self.pidfd.has_exited();
        if !matches!(observation, Ok(false)) {
            // When the castle is poisoned, we continue the fence, not give up resources.
            let mut scheduler = scheduler
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let leases = scheduler.fence_stopped_worker(self.worker, self.epoch);
            self.stage = WatchStage::Fenced { leases };
        }
        observation.map(|_| self.stage)
    }

    /// Serializes a local scheduler solution with the observation of a retained process. The
    /// mere match between worker and pidfd does not confirm origin: it is the responsibility
    /// of the launcher to establish it before the observer is created.
    ///
    /// Death between two checks doesn't return return returns to the caller: even the already
    /// issued set is guarded as indefinite, and after the return, independent oversight of
    /// the process must continue.
    pub(crate) fn acquire_if_alive(
        &mut self,
        scheduler: &Mutex<Scheduler>,
        request: Request,
        lease: LeaseId,
    ) -> Result<GrantResult, SysError> {
        if self.stage != WatchStage::Watching
            || request.owner.worker != self.worker
            || request.owner.authenticator.device_epoch != self.epoch
        {
            return Ok(GrantResult::Denied);
        }

        let mut scheduler = scheduler
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let before = self.pidfd.has_exited();
        if !matches!(before, Ok(false)) {
            let leases = scheduler.fence_stopped_worker(self.worker, self.epoch);
            self.stage = WatchStage::Fenced { leases };
            return before.map(|_| GrantResult::Denied);
        }

        let result = scheduler.acquire(request, lease);
        let after = self.pidfd.has_exited();
        if !matches!(after, Ok(false)) {
            let leases = scheduler.fence_stopped_worker(self.worker, self.epoch);
            self.stage = WatchStage::Fenced { leases };
            return after.map(|_| GrantResult::Denied);
        }
        Ok(result)
    }
}
