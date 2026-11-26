use super::CleanupScope;

pub(crate) fn relay_scope(
    scope: &CleanupScope,
) -> carosign_protocol::carosign::broker_relay::v1::Scope {
    use carosign_core::resource::AccessMode;
    use carosign_protocol::carosign::broker_relay::v1::{AccessMode as WireMode, Claim, Owner, Scope};
    let owner = scope.owner();
    Scope {
        lease_id: scope.lease_id().as_bytes().to_vec(),
        owner: Some(Owner {
            operation_id: owner.operation.as_bytes().to_vec(),
            worker_instance: owner.worker.as_bytes().to_vec(),
            authenticator_id: owner.authenticator.id.as_bytes().to_vec(),
            device_epoch: owner.authenticator.device_epoch.value(),
        }),
        claims: scope
            .resource_set()
            .claims()
            .iter()
            .map(|claim| Claim {
                resource_id: claim.id.as_bytes().to_vec(),
                mode: match claim.mode {
                    AccessMode::Exclusive => WireMode::Exclusive,
                    AccessMode::SharedRead => WireMode::SharedRead,
                    AccessMode::Mediated => WireMode::Mediated,
                } as i32,
            })
            .collect(),
    }
}
