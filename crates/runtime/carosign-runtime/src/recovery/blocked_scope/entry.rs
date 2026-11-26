use crate::operation::actor::OperationActor;
use carosign_core::ids::MutationId;

pub(super) struct Entry {
    pub(super) mutation: MutationId,
    pub(super) actor: Option<OperationActor>,
}
