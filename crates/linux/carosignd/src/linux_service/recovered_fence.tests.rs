use super::*;
use carosign_core::{
    device::AuthenticatorRef,
    ids::{LeaseId, OperationId, WorkerInstanceId},
};
use carosign_persistence::redb::backend::InitialFenceStore;
use std::os::unix::fs::PermissionsExt;

#[test]
fn only_successful_owner_confirmation_releases_durable_fence() {
    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("owner");
    let mut profile = ProviderProfile::device_fixture(owner.clone(), owner.clone());
    profile.set_recovery_owner(owner.clone());
    let boot = BootId::from_bytes([1; 16]).unwrap();
    let scope = FenceScope {
        lease: LeaseId::from_bytes([2; 16]).unwrap(),
        operation: OperationId::from_bytes([3; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([4; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: profile.identity().authenticator,
            device_epoch: profile.epoch(),
        },
        resources: profile.identity().resources.clone(),
    };
    let path = directory.path().join("fences.redb");
    let store = InitialFenceStore::create_new(&path).unwrap();
    store.fence_scoped(&scope).unwrap();
    let resources = LocalResourceOwner::from_initial_fence_store(store).unwrap();

    std::fs::write(&owner, "#!/bin/sh\nexit 10\n").unwrap();
    std::fs::set_permissions(&owner, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(recover(&profile, &resources, boot).is_err());
    assert!(resources.has_recovered_fence(&scope.resources).unwrap());

    let resource = hex(scope.resources.claims()[0].id.as_bytes());
    std::fs::write(
        &owner,
        format!(
            "#!/bin/sh\ntest \"$1 $2 $3 ${{16}} ${{17}}\" = 'recover-resource-v1 --reason unconfirmed-release --resource {resource}:exclusive'\n"
        ),
    )
    .unwrap();
    let other = ProviderProfile::device_fixture(owner.clone(), owner);
    recover(&other, &resources, boot).unwrap();
    assert!(resources.has_recovered_fence(&scope.resources).unwrap());

    recover(&profile, &resources, boot).unwrap();
    assert!(!resources.has_recovered_fence(&scope.resources).unwrap());
    drop(resources);
    assert!(
        InitialFenceStore::open_guarded(&path)
            .unwrap()
            .fences()
            .unwrap()
            .is_empty()
    );
}
