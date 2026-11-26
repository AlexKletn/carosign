use super::super::ExpiredRequest;
use super::super::FenceStage;
use super::super::GrantResult;
use super::super::Holder;
use super::super::Priority;
use super::super::QUEUE_MAX;
use super::super::Quiescing;
use super::super::Request;
use super::super::cleanup;
use super::super::is_starving;
use super::DurableStore;
use super::FenceLifetime;
use super::RestoreFenceError;
use crate::ports::ResourceOwner;
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::LeaseId;
use carosign_core::ids::OperationId;
use carosign_core::operation::context::BootInstant;
use carosign_core::resource::AccessMode;
use carosign_core::resource::ResourceSet;

/// Scheduler.
#[derive(Debug, Default)]
pub struct Scheduler {
    holders: std::collections::BTreeMap<carosign_core::ids::ResourceId, Vec<Holder>>,
    // Recovered resources after a failure do not have a living owner of the issue.
    recovered_fences: std::collections::BTreeSet<carosign_core::ids::ResourceId>,
    // Blocking the source should live as long as all links to the scheduler.
    fence_lifetime: Option<FenceLifetime>,
    durable_store: Option<DurableStore>,
    durable_leases: std::collections::BTreeSet<LeaseId>,
    requests_started: bool,
    // The fencing applies to all issuances, including shared resources.
    fences: std::collections::BTreeMap<LeaseId, FenceStage>,
    fence_keys: std::collections::BTreeMap<LeaseId, std::sync::Arc<()>>,
    // The era is preserved at the setting, and not assigned at the issuance.
    queue: Vec<(Request, LeaseId, DeviceEpoch)>,
    // Issues after cleaning are retained until receipt by the responsible composition.
    cleanup_grants: Vec<(LeaseId, OperationId)>,
    epoch: Option<DeviceEpoch>,
    authenticator_epochs:
        std::collections::BTreeMap<carosign_core::ids::AuthenticatorId, DeviceEpoch>,
    stopped_workers:
        std::collections::BTreeSet<(carosign_core::ids::WorkerInstanceId, DeviceEpoch)>,
    // A couple of processes of this worker ended and this was observed by the composition.
    exited_workers: std::collections::BTreeSet<carosign_core::ids::WorkerInstanceId>,
    // When the memory is exhausted, the fences of issuance are closed completely until
    // restoration.
    stopped_capacity_exhausted: bool,
}

impl Scheduler {
    /// An empty scheduler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Restores the fence before the issuance and request.
    /// The source of the record and the proof of its stability checks the caller.
    /// The fence cannot be removed by the change of era or the completion of processes.
    pub fn restore_fences(
        &mut self,
        resources: impl IntoIterator<Item = carosign_core::ids::ResourceId>,
    ) -> Result<(), RestoreFenceError> {
        if self.requests_started {
            return Err(RestoreFenceError);
        }
        self.recovered_fences.extend(resources);
        Ok(())
    }

    /// Restores fences and holds their source until the scheduler is destroyed.
    /// The caller checks the source and stability of the records before the call.
    pub fn restore_fences_with_owner(
        &mut self,
        resources: impl IntoIterator<Item = carosign_core::ids::ResourceId>,
        owner: impl Send + Sync + 'static,
    ) -> Result<(), RestoreFenceError> {
        if self.requests_started || self.fence_lifetime.is_some() || self.durable_store.is_some() {
            return Err(RestoreFenceError);
        }
        self.recovered_fences.extend(resources);
        self.fence_lifetime = Some(FenceLifetime {
            _owner: Box::new(owner),
        });
        Ok(())
    }

    /// Holds a proven source of fences along with the scheduler.
    pub fn restore_durable_fences(
        &mut self,
        resources: impl IntoIterator<Item = carosign_core::ids::ResourceId>,
        store: std::sync::Arc<carosign_persistence::redb::backend::InitialFenceStore>,
    ) -> Result<(), RestoreFenceError> {
        if self.requests_started || self.fence_lifetime.is_some() || self.durable_store.is_some() {
            return Err(RestoreFenceError);
        }
        self.recovered_fences.extend(resources);
        self.durable_store = Some(DurableStore(store));
        Ok(())
    }

    /// Sustainablely protects accurate live output prior to hardware FD transmission.
    /// Any mistake, it stays locked in memory, and the outcome of the recording may not be
    /// known, and this method does not allow FD transmission by itself.
    pub fn prepare_durable_fence(
        &mut self,
        lease: LeaseId,
        owner: ResourceOwner,
        set: &ResourceSet,
    ) -> Result<(), carosign_persistence::model::StorageError> {
        use carosign_persistence::model::StorageError;
        use carosign_persistence::redb::backend::FenceScope;
        if !self.holds_live_scope(lease, owner, set) || self.durable_leases.contains(&lease) {
            return Err(StorageError::Unavailable);
        }
        let store = self
            .durable_store
            .as_ref()
            .ok_or(StorageError::Unavailable)?
            .0
            .clone();
        self.durable_leases.insert(lease);
        store.fence_scoped(&FenceScope {
            lease,
            operation: owner.operation,
            worker: owner.worker,
            authenticator: owner.authenticator,
            resources: set.clone(),
        })?;
        Ok(())
    }

    /// Remembers the era of the device: an outdated message does not bring it back.
    pub fn set_epoch(&mut self, epoch: DeviceEpoch) {
        if self.epoch.is_none_or(|current| epoch >= current) {
            self.epoch = Some(epoch);
            for current in self.authenticator_epochs.values_mut() {
                *current = (*current).max(epoch);
            }
        }
    }

    pub fn set_authenticator_epoch(
        &mut self,
        authenticator: carosign_core::device::AuthenticatorRef,
    ) {
        let epoch = self
            .epoch_for(authenticator.id)
            .map_or(authenticator.device_epoch, |current| {
                current.max(authenticator.device_epoch)
            });
        self.authenticator_epochs.insert(authenticator.id, epoch);
    }

    fn epoch_for(&self, id: carosign_core::ids::AuthenticatorId) -> Option<DeviceEpoch> {
        self.authenticator_epochs.get(&id).copied().or(self.epoch)
    }

    /// Asks for a set.
    ///
    /// ⚠️ The decision is made **over the entire set **, not by resources in turn: otherwise
    /// the two waiting would hold half each other.
    pub fn acquire(&mut self, request: Request, lease: LeaseId) -> GrantResult {
        self.requests_started = true;
        let Some(epoch) = self.epoch_for(request.owner.authenticator.id) else {
            return GrantResult::Denied;
        };
        // Incompatible modes on one resource in one set are impossible: `ResourceSet` It
        // doesn't miss it, but we'll check again.
        if self.stopped_capacity_exhausted
            || self.exited_workers.contains(&request.owner.worker)
            || self
                .stopped_workers
                .contains(&(request.owner.worker, epoch))
            || !compatible(&request.set)
            || request.owner.authenticator.device_epoch != epoch
        {
            return GrantResult::Denied;
        }
        if self.lease_in_use(lease) || self.operation_in_use(request.owner.operation) {
            return GrantResult::Denied;
        }

        let earlier_in_class = self.queue.iter().any(|(queued, _, queued_epoch)| {
            Some(*queued_epoch) == self.epoch_for(queued.owner.authenticator.id)
                && queued.priority == request.priority
                && !self
                    .stopped_workers
                    .contains(&(queued.owner.worker, *queued_epoch))
        });
        if !earlier_in_class && self.can_grant(&request.set) {
            self.issue(&request.set, request.owner, lease, epoch);
            return GrantResult::Granted { lease };
        }

        if self.queue.len() >= QUEUE_MAX {
            return GrantResult::Denied;
        }
        self.queue.push((request, lease, epoch));
        self.resort();
        GrantResult::Queued
    }

    /// Remembers the confirmed completion of both processes.
    /// The caller must receive confirmation from the OS, not from the worker.
    pub fn mark_processes_exited(&mut self, worker: carosign_core::ids::WorkerInstanceId) {
        self.exited_workers.insert(worker);
    }

    /// Only the coincidental issuance of the completed worker is released.
    pub fn release_after_processes_exit(
        &mut self,
        lease: LeaseId,
        owner: ResourceOwner,
        set: &ResourceSet,
    ) -> bool {
        if !self.exited_workers.contains(&owner.worker)
            || !self.matches_scope(lease, owner, set)
            || self.fences.contains_key(&lease)
            || self.durable_leases.contains(&lease)
        {
            return false;
        }
        self.release(lease);
        true
    }

    /// Releases the issue and gives the queue.
    ///
    /// Old or unknown rendition does not exempt. The current contract still does not transfer
    /// the era and the owner of the caller and does not confirm the cleaning.
    pub fn release(&mut self, lease: LeaseId) -> Vec<(LeaseId, OperationId)> {
        self.release_inner(lease, None)
    }

    /// Releases the issue taking into account the verified time of the same load.
    pub fn release_at(&mut self, lease: LeaseId, now: BootInstant) -> Vec<(LeaseId, OperationId)> {
        self.release_inner(lease, Some(now))
    }

    fn release_inner(
        &mut self,
        lease: LeaseId,
        now: Option<BootInstant>,
    ) -> Vec<(LeaseId, OperationId)> {
        if self.fences.contains_key(&lease) || self.durable_leases.contains(&lease) {
            return Vec::new();
        }
        let mut matched = false;
        for holder in self.holders.values().flatten().filter(|h| h.lease == lease) {
            matched = true;
            if Some(holder.epoch) != self.epoch_for(holder.owner.authenticator.id) {
                return Vec::new();
            }
        }
        if !matched {
            return Vec::new();
        }
        self.holders.retain(|_, holders| {
            holders.retain(|holder| holder.lease != lease);
            !holders.is_empty()
        });
        match now {
            Some(now) => self.dispatch_at(now),
            None => self.dispatch(),
        }
    }

    /// Protects the exact active issuance until any mediator waits.
    /// The coincidence of the area does not prove the origin of the issuance or peace.
    pub fn begin_quiescing(
        &mut self,
        lease: LeaseId,
        owner: ResourceOwner,
        set: &ResourceSet,
    ) -> Option<Quiescing> {
        if self.epoch_for(owner.authenticator.id) != Some(owner.authenticator.device_epoch)
            || self.fences.contains_key(&lease)
        {
            return None;
        }
        if !self.matches_scope(lease, owner, set) {
            return None;
        }
        self.fences.insert(lease, FenceStage::Quiescing);
        let key = std::sync::Arc::new(());
        self.fence_keys.insert(lease, key.clone());
        Some(Quiescing {
            lease,
            owner,
            set: set.clone(),
            key,
        })
    }

    /// Checks the area being held in the current scheduler shot.
    ///
    /// The result does not certify the source of the issue and is not a permit: verification
    /// of the origin of the worker and serialization with subsequent action remain the
    /// responsibility of the trusted owner.
    pub fn holds_live_scope(
        &self,
        lease: LeaseId,
        owner: ResourceOwner,
        set: &ResourceSet,
    ) -> bool {
        !self.stopped_capacity_exhausted
            && !self.exited_workers.contains(&owner.worker)
            && self.epoch_for(owner.authenticator.id) == Some(owner.authenticator.device_epoch)
            && !self
                .stopped_workers
                .contains(&(owner.worker, owner.authenticator.device_epoch))
            && !self.fences.contains_key(&lease)
            && self.matches_scope(lease, owner, set)
    }

    fn matches_scope(&self, lease: LeaseId, owner: ResourceOwner, set: &ResourceSet) -> bool {
        let mut actual = Vec::new();
        for (id, holders) in &self.holders {
            for holder in holders.iter().filter(|holder| holder.lease == lease) {
                if holder.owner != owner || holder.epoch != owner.authenticator.device_epoch {
                    return false;
                }
                actual.push(carosign_core::resource::ResourceClaim {
                    id: *id,
                    mode: holder.mode,
                });
            }
        }
        ResourceSet::new(actual).is_ok_and(|actual| &actual == set)
    }

    /// Verifies the withheld output, including the area under cleaning.
    pub fn holds_scope(&self, lease: LeaseId, owner: ResourceOwner, set: &ResourceSet) -> bool {
        self.matches_scope(lease, owner, set)
    }

    /// It protects indefinite issuance only when the area is completely coincident.
    pub fn mark_uncertain_scope(
        &mut self,
        lease: LeaseId,
        owner: ResourceOwner,
        set: &ResourceSet,
    ) -> bool {
        if !self.matches_scope(lease, owner, set) {
            return false;
        }
        self.fences.insert(lease, FenceStage::Uncertain);
        true
    }

    /// Protects all withheld extradition of the deceased worker of the specified era.
    ///
    /// The death of the process does not confirm the rest of the devices: the usual release
    /// and previously started cleaning no longer remove the fence.
    /// Applications are retained for notification through `take_expired_requests`If there's a
    /// shortage of space, all the issuances are closed.
    pub fn fence_stopped_worker(
        &mut self,
        worker: carosign_core::ids::WorkerInstanceId,
        epoch: DeviceEpoch,
    ) -> usize {
        const STOPPED_MAX: usize = 256;
        if !self.stopped_workers.contains(&(worker, epoch)) {
            if self.stopped_workers.len() == STOPPED_MAX {
                self.stopped_capacity_exhausted = true;
            } else {
                self.stopped_workers.insert((worker, epoch));
            }
        }
        let leases: std::collections::BTreeSet<_> = self
            .holders
            .values()
            .flatten()
            .filter(|holder| holder.owner.worker == worker && holder.epoch == epoch)
            .map(|holder| holder.lease)
            .collect();
        for lease in &leases {
            self.fences.insert(*lease, FenceStage::Uncertain);
        }
        leases.len()
    }

    /// An indefinite answer does not remove the fencing or exempt extradition.
    pub fn quiescing_uncertain(&mut self, held: Quiescing) -> bool {
        if self.fences.get(&held.lease) != Some(&FenceStage::Quiescing)
            || self
                .fence_keys
                .get(&held.lease)
                .is_none_or(|key| !std::sync::Arc::ptr_eq(key, &held.key))
        {
            return false;
        }
        self.fences.insert(held.lease, FenceStage::Uncertain);
        true
    }

    /// Remove the fence only after the confirmed release of the same area.
    pub fn finish_quiescing(
        &mut self,
        held: Quiescing,
        released: cleanup::Released,
    ) -> Result<(), Quiescing> {
        if self.durable_leases.contains(&held.lease) {
            let Some(store) = self.durable_store.as_ref().map(|store| store.0.clone()) else {
                return Err(held);
            };
            return self.finish_quiescing_durable_inner(held, released, &store, None);
        }
        self.finish_quiescing_inner(held, released, None)
    }

    /// Completes the cleaning and takes into account the waiting of applications for the time
    /// of the current load.
    pub fn finish_quiescing_at(
        &mut self,
        held: Quiescing,
        released: cleanup::Released,
        now: BootInstant,
    ) -> Result<(), Quiescing> {
        if self.durable_leases.contains(&held.lease) {
            let Some(store) = self.durable_store.as_ref().map(|store| store.0.clone()) else {
                return Err(held);
            };
            return self.finish_quiescing_durable_inner(held, released, &store, Some(now));
        }
        self.finish_quiescing_inner(held, released, Some(now))
    }

    /// Releases precisely held output first steadily, then in memory.
    /// Reconciliation of the live scheduler and recording are performed with sole access to
    /// it; the error saves the operational fence.
    pub fn finish_quiescing_durable(
        &mut self,
        held: Quiescing,
        released: cleanup::Released,
        store: &carosign_persistence::redb::backend::InitialFenceStore,
    ) -> Result<(), Quiescing> {
        self.finish_quiescing_durable_inner(held, released, store, None)
    }

    fn finish_quiescing_durable_inner(
        &mut self,
        held: Quiescing,
        released: cleanup::Released,
        store: &carosign_persistence::redb::backend::InitialFenceStore,
        now: Option<BootInstant>,
    ) -> Result<(), Quiescing> {
        if !self.durable_leases.contains(&held.lease)
            || self
                .durable_store
                .as_ref()
                .is_none_or(|owned| !core::ptr::eq(owned.0.as_ref(), store))
        {
            return Err(held);
        }
        if !self.can_finish_quiescing(&held, &released) {
            return Err(held);
        }
        let Ok(released) = released.release_durable_fence(store, &held) else {
            return Err(held);
        };
        self.durable_leases.remove(&held.lease);
        self.finish_quiescing_inner(held, released, now)
    }

    fn can_finish_quiescing(&self, held: &Quiescing, released: &cleanup::Released) -> bool {
        self.epoch_for(held.owner.authenticator.id) == Some(held.owner.authenticator.device_epoch)
            && self.fences.get(&held.lease) == Some(&FenceStage::Quiescing)
            && self.matches_scope(held.lease, held.owner, &held.set)
            && self
                .fence_keys
                .get(&held.lease)
                .is_some_and(|key| std::sync::Arc::ptr_eq(key, &held.key))
            && released.matches(held.lease, held.owner, &held.set, &held.key)
    }

    fn finish_quiescing_inner(
        &mut self,
        held: Quiescing,
        released: cleanup::Released,
        now: Option<BootInstant>,
    ) -> Result<(), Quiescing> {
        if !self.can_finish_quiescing(&held, &released) {
            return Err(held);
        }
        self.fences.remove(&held.lease);
        self.fence_keys.remove(&held.lease);
        self.holders.retain(|_, holders| {
            holders.retain(|holder| holder.lease != held.lease);
            !holders.is_empty()
        });
        // Until the previous batch is accepted, we do not create new notifications:
        // otherwise, with a non-working recipient, the queue of results would grow without
        // limit.
        if self.cleanup_grants.is_empty() {
            self.cleanup_grants = match now {
                Some(now) => self.dispatch_at(now),
                None => self.dispatch(),
            };
        }
        Ok(())
    }

    /// Once transfers the saved issue after confirmed cleaning.
    /// An application issued but not yet transmitted from the previous era is held until
    /// reinstatement: it cannot be issued to the caller as current.
    #[must_use]
    pub fn take_cleanup_grants(&mut self) -> Vec<(LeaseId, OperationId)> {
        self.take_cleanup_grants_inner(None)
    }

    /// Transmits the results of cleaning and takes into account the time at the next issue.
    #[must_use]
    pub fn take_cleanup_grants_at(&mut self, now: BootInstant) -> Vec<(LeaseId, OperationId)> {
        self.take_cleanup_grants_inner(Some(now))
    }

    fn take_cleanup_grants_inner(
        &mut self,
        now: Option<BootInstant>,
    ) -> Vec<(LeaseId, OperationId)> {
        let mut granted = Vec::new();
        for (lease, operation) in std::mem::take(&mut self.cleanup_grants) {
            if !self.fences.contains_key(&lease)
                && self.holders.values().flatten().any(|holder| {
                    holder.lease == lease
                        && holder.owner.operation == operation
                        && Some(holder.epoch) == self.epoch_for(holder.owner.authenticator.id)
                })
            {
                granted.push((lease, operation));
            } else {
                self.cleanup_grants.push((lease, operation));
            }
        }
        // While the previous results were not taken away, the issuance of the queue was
        // postponed.
        // Once received, we review pending applications again.
        if self.cleanup_grants.is_empty() {
            granted.extend(match now {
                Some(now) => self.dispatch_at(now),
                None => self.dispatch(),
            });
        }
        granted
    }

    /// The current stage of the fencing; not evidence of physical quiescence.
    pub fn fence_stage(&self, lease: LeaseId) -> Option<FenceStage> {
        self.fences.get(&lease).copied()
    }

    /// There's a lot in line.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Cancels only a pending application with a matching transaction and number.
    ///
    /// The active issue is not exempted. After cancellation, the caller separately calls
    /// `dispatch`When you are ready to accept the results of the issue.
    pub fn cancel_queued(&mut self, operation: OperationId, lease: LeaseId) -> bool {
        let Some(index) = self.queue.iter().position(|(request, queued_lease, _)| {
            request.owner.operation == operation && *queued_lease == lease
        }) else {
            return false;
        };
        self.queue.remove(index);
        true
    }

    /// Retrieves applications from previous eras to notify owners.
    ///
    /// Active issuances do not change: the new era does not prove their cleaning.
    /// Returned applications are subject to processing as unissued.
    #[must_use]
    pub fn take_expired_requests(&mut self) -> Vec<ExpiredRequest> {
        let epoch = self.epoch;
        let authenticator_epochs = &self.authenticator_epochs;
        let mut expired = Vec::new();
        self.queue.retain(|(request, lease, queued_epoch)| {
            if Some(*queued_epoch)
                == authenticator_epochs
                    .get(&request.owner.authenticator.id)
                    .copied()
                    .or(epoch)
                && !self.stopped_capacity_exhausted
                && !self
                    .stopped_workers
                    .contains(&(request.owner.worker, *queued_epoch))
            {
                true
            } else {
                expired.push(ExpiredRequest {
                    request: request.clone(),
                    lease: *lease,
                    epoch: *queued_epoch,
                });
                false
            }
        });
        expired
    }

    /// Who holds the resource (for inspections)?
    pub fn holder_of(&self, resource: carosign_core::ids::ResourceId) -> Option<OperationId> {
        self.holders
            .get(&resource)
            .and_then(|holders| holders.first())
            .map(|holder| holder.owner.operation)
    }

    pub fn has_recovered_fence(&self, set: &ResourceSet) -> bool {
        set.claims()
            .iter()
            .any(|claim| self.recovered_fences.contains(&claim.id))
    }

    pub fn recovered_scopes(
        &self,
    ) -> Result<
        Vec<carosign_persistence::redb::backend::FenceScope>,
        carosign_persistence::model::StorageError,
    > {
        self.durable_store.as_ref().map_or_else(
            || Ok(Vec::new()),
            |store| {
                let mut scopes = store.0.scoped_fences()?;
                scopes.retain(|scope| {
                    scope
                        .resources
                        .claims()
                        .iter()
                        .all(|claim| self.recovered_fences.contains(&claim.id))
                });
                Ok(scopes)
            },
        )
    }

    pub fn release_recovered_fence(
        &mut self,
        permit: carosign_persistence::redb::backend::DurableReleasePermit,
    ) -> Result<(), carosign_persistence::model::StorageError> {
        use carosign_persistence::model::StorageError;

        let resources = permit.scope().resources.clone();
        let store = self
            .durable_store
            .as_ref()
            .ok_or(StorageError::Unavailable)?;
        if !resources
            .claims()
            .iter()
            .all(|claim| self.recovered_fences.contains(&claim.id))
        {
            return Err(StorageError::Unavailable);
        }
        store.0.release_verified(permit)?;
        for claim in resources.claims() {
            self.recovered_fences.remove(&claim.id);
        }
        Ok(())
    }

    /// Can the set be issued right now?
    fn can_grant(&self, set: &ResourceSet) -> bool {
        set.claims().iter().all(|claim| {
            !self.recovered_fences.contains(&claim.id)
                && match self.holders.get(&claim.id) {
                    None => true,
                    Some(holders) => holders.iter().all(|holder| {
                        Some(holder.epoch) == self.epoch_for(holder.owner.authenticator.id)
                            && !self.fences.contains_key(&holder.lease)
                            && match holder.mode {
                                AccessMode::Exclusive => false,
                                AccessMode::SharedRead => claim.mode == AccessMode::SharedRead,
                                AccessMode::Mediated => false,
                            }
                    }),
                }
        })
    }

    fn issue(
        &mut self,
        set: &ResourceSet,
        owner: ResourceOwner,
        lease: LeaseId,
        epoch: DeviceEpoch,
    ) {
        for claim in set.claims() {
            self.holders.entry(claim.id).or_default().push(Holder {
                lease,
                owner,
                epoch,
                mode: claim.mode,
            });
        }
    }

    fn lease_in_use(&self, lease: LeaseId) -> bool {
        self.holders
            .values()
            .flatten()
            .any(|holder| holder.lease == lease)
            || self.queue.iter().any(|(_, queued, _)| *queued == lease)
    }

    fn operation_in_use(&self, operation: OperationId) -> bool {
        self.holders
            .values()
            .flatten()
            .any(|holder| holder.owner.operation == operation)
            || self
                .queue
                .iter()
                .any(|(request, _, _)| request.owner.operation == operation)
    }

    /// It gives a line: first class, then the order of production.
    ///
    /// ⚠️ Priority raising after waiting has not yet been implemented.
    #[must_use]
    pub fn dispatch(&mut self) -> Vec<(LeaseId, OperationId)> {
        if self.stopped_capacity_exhausted {
            return Vec::new();
        }
        let mut granted = Vec::new();
        let mut index = 0;
        while index < self.queue.len() {
            let (request, lease, queued_epoch) = self.queue[index].clone();
            if self
                .stopped_workers
                .contains(&(request.owner.worker, queued_epoch))
            {
                index += 1;
                continue;
            }
            let earlier_in_class = self.queue[..index]
                .iter()
                .any(|(earlier, _, earlier_epoch)| {
                    Some(*earlier_epoch) == self.epoch_for(earlier.owner.authenticator.id)
                        && earlier.priority == request.priority
                        && !self
                            .stopped_workers
                            .contains(&(earlier.owner.worker, *earlier_epoch))
                });
            if Some(queued_epoch) == self.epoch_for(request.owner.authenticator.id)
                && !earlier_in_class
                && self.can_grant(&request.set)
            {
                self.issue(&request.set, request.owner, lease, queued_epoch);
                granted.push((lease, request.owner.operation));
                self.queue.remove(index);
            } else {
                index += 1;
            }
        }
        granted
    }

    /// Gives out a queue with due regard to time to prevent starvation.
    ///
    /// The upgrade only applies to a pending interactive request for the same download and
    /// does not change resource compatibility or existing fencing.
    /// Order within each class remains the order of queueing.
    #[must_use]
    pub fn dispatch_at(&mut self, now: BootInstant) -> Vec<(LeaseId, OperationId)> {
        let stopped = &self.stopped_workers;
        let first_interactive = self.queue.iter().find(|(request, _, queued_epoch)| {
            request.priority == Priority::InteractiveManage
                && Some(*queued_epoch) == self.epoch_for(request.owner.authenticator.id)
                && !stopped.contains(&(request.owner.worker, *queued_epoch))
        });
        let promote = first_interactive
            .filter(|(request, _, _)| is_starving(now, request))
            .map(|(_, lease, _)| *lease);
        self.queue.sort_by_key(|(request, lease, _)| {
            (
                Some(*lease) != promote || promote.is_none(),
                request.priority,
            )
        });
        let granted = self.dispatch();
        self.resort();
        granted
    }

    fn resort(&mut self) {
        // Sustainable sorting preserves the actual order of staging within the class, even if
        // the time marks coincide or go in discrepancies.
        self.queue.sort_by_key(|(request, _, _)| request.priority);
    }
}

fn compatible(set: &ResourceSet) -> bool {
    let mut seen: Vec<(carosign_core::ids::ResourceId, AccessMode)> = Vec::new();
    for claim in set.claims() {
        if seen.iter().any(|(id, _)| *id == claim.id) {
            return false;
        }
        seen.push((claim.id, claim.mode));
    }
    true
}
