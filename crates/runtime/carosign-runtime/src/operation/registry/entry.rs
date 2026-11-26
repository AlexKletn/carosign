use super::Owner;
use crate::operation::actor::OperationActor;

#[derive(Debug)]
pub(super) struct Entry {
    pub(super) actor: OperationActor,
    pub(super) owner: Owner,
}
