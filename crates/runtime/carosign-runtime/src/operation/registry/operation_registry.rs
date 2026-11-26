use super::{Entry, OPERATIONS_MAX, Owner, RegistryError, Slot, TransferError};
use crate::operation::actor::OperationActor;
use crate::recovery::blocked_scope::RecoveryCoordinator;
use carosign_core::ids::OperationId;

/// Transaction register.
#[derive(Debug, Default)]
pub struct OperationRegistry {
    // Place and number are saved both at the step and when transferring to the
    // reconciliation.
    operations: std::collections::BTreeMap<OperationId, Slot>,
}

impl OperationRegistry {
    /// Empty register.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds surgery.
    ///
    /// ⚠️ The limit is checked by the **do** insert: a queue without a boundary is a memory
    /// failure under load, not a "well let it."
    pub fn insert(
        &mut self,
        actor: OperationActor,
        owner: Owner,
    ) -> Result<OperationId, RegistryError> {
        let id = actor.state().common.context.operation_id;
        if self.operations.contains_key(&id) {
            return Err(RegistryError::Duplicate);
        }
        if self.operations.len() >= OPERATIONS_MAX {
            return Err(RegistryError::TooMany);
        }
        self.operations
            .insert(id, Slot::Present(Box::new(Entry { actor, owner })));
        Ok(id)
    }

    /// Gives an actor a step time.
    ///
    /// ⚠️ Withdraws from the register for the time of the step: at the same time, two will
    /// not approach one operation.
    pub fn take(&mut self, id: OperationId) -> Result<(OperationActor, Owner), RegistryError> {
        let slot = self
            .operations
            .get_mut(&id)
            .ok_or(RegistryError::NotFound)?;
        if !matches!(slot, Slot::Present(_)) {
            return Err(RegistryError::NotFound);
        }
        let Slot::Present(entry) = std::mem::replace(slot, Slot::CheckedOut) else {
            unreachable!("состояние проверено")
        };
        let Entry { actor, owner } = *entry;
        Ok((actor, owner))
    }

    /// Brings the actor back.
    pub fn put(&mut self, actor: OperationActor, owner: Owner) {
        let id = actor.state().common.context.operation_id;
        let slot = self
            .operations
            .get_mut(&id)
            .expect("актёр не изъят из реестра");
        assert!(
            matches!(slot, Slot::CheckedOut),
            "актёр уже возвращён в реестр"
        );
        *slot = Slot::Present(Box::new(Entry { actor, owner }));
    }

    /// Transfers the actor for reconciliation, keeping the number and owner in the register.
    /// If the coordinator refuses, the actor returns with a queue to the same slot.
    pub fn transfer_to_recovery(
        &mut self,
        id: OperationId,
        coordinator: &mut RecoveryCoordinator,
    ) -> Result<(), TransferError> {
        let (actor, owner) = self.take(id).map_err(TransferError::Registry)?;
        match coordinator.register(actor) {
            Ok(()) => {
                *self.operations.get_mut(&id).expect("слот зарезервирован") =
                    Slot::Recovering(owner);
                Ok(())
            }
            Err(failure) => {
                self.put(*failure.actor, owner);
                Err(TransferError::Recovery(failure.reason))
            }
        }
    }

    /// The owner of the transaction, transferred to the coordinator, for subsequent
    /// verification of rights.
    pub fn recovery_owner(&self, id: OperationId) -> Option<&Owner> {
        match self.operations.get(&id) {
            Some(Slot::Recovering(owner)) => Some(owner),
            _ => None,
        }
    }

    /// Refuses removal until the verified cleaning completion is connected.
    /// The terminal phase alone does not confirm the rest of the resources or the stable
    /// outcome of the mutation.
    pub fn remove(&mut self, id: OperationId) -> Result<(), RegistryError> {
        if !matches!(self.operations.get(&id), Some(Slot::Present(_))) {
            return Err(RegistryError::NotFound);
        }
        Err(RegistryError::CleanupUnverified)
    }

    /// So many surgeries.
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// The register is empty.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Is there such an operation?
    pub fn contains(&self, id: OperationId) -> bool {
        self.operations.contains_key(&id)
    }
}
