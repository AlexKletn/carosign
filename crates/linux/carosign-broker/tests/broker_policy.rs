//! Mediator: policy, extradition, descriptors and refusal.
//!
//! ⚠️ It checks what makes the broker a hole if it makes a mistake: a resource not from
//! the issue, an alien role, an outdated era that changes the call without permission on the
//! disk, the fall of the broker and “withdrawal means taken away.”

use carosign_broker::fd_policy::{self, FdPlanError, Handoff, Revocation};
use carosign_broker::policy::{Allowance, Handler, Policy, PolicyError};
use carosign_broker::recovery::{AfterCrash, WorkerAfterDeath};
use carosign_broker::resources::{
    BindingContext, Claim, ClaimError, FdRole, ResourceKind, role_fits, validate,
};
use carosign_broker::server::{Broker, Call, CallResult, DispatchAuthorization};
use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_core::ids::{
    AuthenticatorId, CommandId, LeaseId, MutationId, OperationId, PackageId, ResourceId,
    WorkerInstanceId,
};

fn id(byte: u8) -> [u8; 16] {
    [byte; 16]
}

fn resource(byte: u8) -> ResourceId {
    ResourceId::from_bytes(id(byte)).expect("валид")
}

fn package() -> PackageId {
    PackageId::from_bytes(id(9)).expect("валид")
}

fn lease() -> LeaseId {
    LeaseId::from_bytes(id(7)).expect("валид")
}

fn binding_context() -> BindingContext {
    BindingContext {
        lease: lease(),
        worker: WorkerInstanceId::from_bytes(id(8)).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes(id(6)).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    }
}

fn claim(byte: u8, role: FdRole) -> Claim {
    Claim {
        resource: resource(byte),
        role,
        kind: match role {
            FdRole::Device => ResourceKind::CharacterDevice,
            FdRole::Event => ResourceKind::EventFd,
            FdRole::Config => ResourceKind::SealedMemfd,
            FdRole::MediatedService => ResourceKind::SocketPair,
        },
    }
}

struct Armed(bool);

impl DispatchAuthorization for Armed {
    fn is_armed(
        &self,
        _operation: OperationId,
        _mutation: MutationId,
        _command: CommandId,
    ) -> bool {
        self.0
    }
}

#[test]
fn roles_are_not_interchangeable() {
    assert!(role_fits(FdRole::Device, ResourceKind::CharacterDevice));
    assert!(!role_fits(FdRole::Device, ResourceKind::SealedMemfd));
    // ⚠️ The mediator does not hand over the raw device under the guise of a channel: it
    // would be a circumvention of politics.
    assert!(!role_fits(
        FdRole::MediatedService,
        ResourceKind::CharacterDevice
    ));
}

#[test]
fn claim_outside_lease_is_refused() {
    let claims = vec![claim(1, FdRole::Device)];
    let result = validate(
        &claims,
        binding_context(),
        DeviceEpoch::FIRST,
        &[resource(2)],
    );
    assert_eq!(
        result,
        Err(ClaimError::NotInLease {
            resource: resource(1)
        })
    );
}

#[test]
fn whole_set_or_nothing() {
    let claims = vec![claim(1, FdRole::Device), claim(2, FdRole::Event)];
    // The second resource is not issued, and the first one is not issued.
    let result = validate(
        &claims,
        binding_context(),
        DeviceEpoch::FIRST,
        &[resource(1)],
    );
    assert!(matches!(result, Err(ClaimError::NotInLease { .. })));
}

#[test]
fn duplicate_resource_is_refused() {
    let claims = vec![claim(1, FdRole::Device), claim(1, FdRole::Device)];
    let result = validate(
        &claims,
        binding_context(),
        DeviceEpoch::FIRST,
        &[resource(1)],
    );
    assert_eq!(
        result,
        Err(ClaimError::Duplicate {
            resource: resource(1)
        })
    );
}

#[test]
fn handoff_counts_must_match() {
    let claims = validate(
        &[claim(1, FdRole::Device)],
        binding_context(),
        DeviceEpoch::FIRST,
        &[resource(1)],
    )
    .expect("выдача");
    let plan = fd_policy::plan(&claims, &[FdRole::Device, FdRole::Event]);
    assert!(matches!(plan, Err(FdPlanError::CountMismatch { .. })));
}

#[test]
fn handoff_rejects_mixed_owner_epoch_and_duplicate_resource() {
    let claims = validate(
        &[claim(1, FdRole::Device), claim(2, FdRole::Event)],
        binding_context(),
        DeviceEpoch::FIRST,
        &[resource(1), resource(2)],
    )
    .unwrap();
    let roles = [FdRole::Device, FdRole::Event];
    assert!(fd_policy::plan(&claims, &roles).is_ok());
    for variant in 0..5 {
        let mut mixed = claims.clone();
        match variant {
            0 => mixed[1].lease = LeaseId::from_bytes(id(11)).unwrap(),
            1 => mixed[1].worker = WorkerInstanceId::from_bytes(id(11)).unwrap(),
            2 => mixed[1].authenticator = AuthenticatorId::from_bytes(id(11)).unwrap(),
            3 => mixed[1].epoch = DeviceEpoch::new(2).unwrap(),
            4 => mixed[1].resource = mixed[0].resource,
            _ => unreachable!(),
        }
        assert!(fd_policy::plan(&mixed, &roles).is_err());
    }
}

#[test]
fn handed_descriptor_is_not_revocable_logically() {
    let claims = validate(
        &[claim(1, FdRole::Device)],
        binding_context(),
        DeviceEpoch::FIRST,
        &[resource(1)],
    )
    .expect("выдача");
    let plan = fd_policy::plan(&claims, &[FdRole::Device]).expect("план");
    assert_eq!(plan.action, Handoff::Deliver);
    // ⚠️ Honest answer: withdrawal of leasing does not terminate access.
    assert_eq!(plan.revocation, Revocation::OnlyByTermination);
}

#[test]
fn binding_uses_requested_worker_and_authenticator_and_rejects_stale_epoch() {
    let context = binding_context();
    let claims = [claim(1, FdRole::Device)];
    let bindings = validate(&claims, context, DeviceEpoch::FIRST, &[resource(1)]).unwrap();
    assert_eq!(bindings[0].lease, context.lease);
    assert_eq!(bindings[0].worker, context.worker);
    assert_eq!(bindings[0].authenticator, context.authenticator.id);
    assert_eq!(bindings[0].epoch, context.authenticator.device_epoch);
    assert_eq!(
        validate(
            &claims,
            context,
            DeviceEpoch::new(2).unwrap(),
            &[resource(1)]
        ),
        Err(ClaimError::StaleEpoch)
    );
}

#[test]
fn mutation_needs_durable_arm() {
    let mut policy = Policy::new();
    policy
        .register(Handler {
            operation_ref: id(5),
            package: package(),
            resource: resource(1),
            may_mutate: true,
            name: "confirm",
        })
        .expect("регистрируется");

    let call = |armed: bool| {
        let armed_check = Armed(armed);
        let broker = Broker::new(&policy, &armed_check, DeviceEpoch::FIRST);
        broker.check(&Call {
            operation_ref: id(5),
            package: package(),
            resource: resource(1),
            epoch: DeviceEpoch::FIRST,
            operation: Some(OperationId::from_bytes(id(1)).expect("валид")),
            mutation: Some(MutationId::from_bytes(id(2)).expect("валид")),
            command: Some(CommandId::from_bytes(id(3)).expect("валид")),
        })
    };

    assert_eq!(call(true), CallResult::Handled);
    // ⚠️ Without a record on a disk - a refusal, although the worker assures that "it is
    // possible."
    assert_eq!(call(false), CallResult::Unauthorized);
}

#[test]
fn mutation_without_ids_is_refused() {
    let mut policy = Policy::new();
    policy
        .register(Handler {
            operation_ref: id(5),
            package: package(),
            resource: resource(1),
            may_mutate: true,
            name: "confirm",
        })
        .expect("регистрируется");
    let armed = Armed(true);
    let broker = Broker::new(&policy, &armed, DeviceEpoch::FIRST);
    let result = broker.check(&Call {
        operation_ref: id(5),
        package: package(),
        resource: resource(1),
        epoch: DeviceEpoch::FIRST,
        operation: None,
        mutation: None,
        command: None,
    });
    assert_eq!(result, CallResult::Unauthorized);
}

#[test]
fn read_only_calls_require_operation_and_exclude_mutation_identifiers() {
    let mut policy = Policy::new();
    policy
        .register(Handler {
            operation_ref: id(5),
            package: package(),
            resource: resource(1),
            may_mutate: false,
            name: "read",
        })
        .unwrap();
    let armed = Armed(true);
    let broker = Broker::new(&policy, &armed, DeviceEpoch::FIRST);
    let mut call = Call {
        operation_ref: id(5),
        package: package(),
        resource: resource(1),
        epoch: DeviceEpoch::FIRST,
        operation: Some(OperationId::from_bytes(id(1)).unwrap()),
        mutation: None,
        command: None,
    };
    assert_eq!(broker.check(&call), CallResult::Handled);
    call.operation = None;
    assert_eq!(broker.check(&call), CallResult::Unauthorized);
    call.operation = Some(OperationId::from_bytes(id(1)).unwrap());
    call.mutation = Some(MutationId::from_bytes(id(2)).unwrap());
    assert_eq!(broker.check(&call), CallResult::Unauthorized);
    call.mutation = None;
    call.command = Some(CommandId::from_bytes(id(3)).unwrap());
    assert_eq!(broker.check(&call), CallResult::Unauthorized);
}

#[test]
fn zero_handler_identifier_cannot_be_registered() {
    let mut policy = Policy::new();
    assert_eq!(
        policy.register(Handler {
            operation_ref: [0; 16],
            package: package(),
            resource: resource(1),
            may_mutate: false,
            name: "invalid",
        }),
        Err(PolicyError::Unsupported)
    );
}

#[test]
fn unknown_handler_is_unsupported_not_passthrough() {
    let policy = Policy::new();
    let armed = Armed(true);
    let broker = Broker::new(&policy, &armed, DeviceEpoch::FIRST);
    let result = broker.check(&Call {
        operation_ref: id(5),
        package: package(),
        resource: resource(1),
        epoch: DeviceEpoch::FIRST,
        operation: Some(OperationId::from_bytes(id(1)).expect("валид")),
        mutation: None,
        command: None,
    });
    assert_eq!(result, CallResult::Unsupported);
}

#[test]
fn stale_epoch_is_refused_first() {
    let mut policy = Policy::new();
    policy
        .register(Handler {
            operation_ref: id(5),
            package: package(),
            resource: resource(1),
            may_mutate: false,
            name: "read",
        })
        .expect("регистрируется");
    let armed = Armed(true);
    let broker = Broker::new(&policy, &armed, DeviceEpoch::FIRST);
    let result = broker.check(&Call {
        operation_ref: id(5),
        package: package(),
        resource: resource(1),
        epoch: DeviceEpoch::FIRST.next().expect("не переполнено"),
        operation: Some(OperationId::from_bytes(id(1)).expect("валид")),
        mutation: None,
        command: None,
    });
    assert_eq!(result, CallResult::Unauthorized);
}

#[test]
fn policy_allows_only_declared_combinations() {
    let mut policy = Policy::new();
    policy
        .allow(Allowance {
            package: package(),
            resource: resource(1),
            role: FdRole::Device,
            kind: ResourceKind::CharacterDevice,
        })
        .expect("добавлено");

    assert!(policy.permitted(package(), resource(1), FdRole::Device));
    assert!(!policy.permitted(package(), resource(1), FdRole::Event));
    assert!(!policy.permitted(package(), resource(2), FdRole::Device));
    assert_eq!(
        policy.allow(Allowance {
            package: package(),
            resource: resource(1),
            role: FdRole::Device,
            kind: ResourceKind::CharacterDevice,
        }),
        Err(PolicyError::Duplicate)
    );
}

#[test]
fn broker_crash_leaves_leases_uncertain() {
    assert_eq!(
        carosign_broker::recovery::after_crash(false),
        AfterCrash::AllUncertain
    );
    assert_eq!(
        carosign_broker::recovery::after_crash(true),
        AfterCrash::KernelClosed
    );
    let leases = vec![lease()];
    assert_eq!(
        carosign_broker::recovery::uncertain_leases(
            &leases,
            carosign_broker::recovery::after_crash(false)
        ),
        leases
    );
    assert_eq!(
        carosign_broker::recovery::uncertain_leases(
            &leases,
            carosign_broker::recovery::after_crash(true)
        ),
        leases,
        "закрытие доступа не подтверждает завершение работы устройства"
    );
}

#[test]
fn termination_revokes_access_but_not_safety() {
    assert_eq!(
        carosign_broker::recovery::revoke_by_termination(true),
        WorkerAfterDeath::AccessRevoked
    );
    assert_eq!(
        carosign_broker::recovery::revoke_by_termination(false),
        WorkerAfterDeath::AccessMayRemain
    );
}

#[test]
fn external_body_accepts_typed_payload_and_checks_fd_count() {
    use carosign_broker::external_body::{
        BodyError, ExternalBody, check_exclusive, check_fd_count,
    };

    // ⚠️ Document 32: Typed arguments go along with the body.
    let body = ExternalBody::for_bytes(&[1u8; 64]).expect("тело");
    assert!(check_exclusive(32, Some(body)).is_ok());
    assert!(check_exclusive(0, Some(body)).is_ok());

    // ⚠️ And just one descriptor, otherwise, close everything and refuse.
    assert!(check_fd_count(Some(body), 1).is_ok());
    assert!(matches!(
        check_fd_count(Some(body), 0),
        Err(BodyError::WrongFdCount { actual: 0 })
    ));
    assert!(matches!(
        check_fd_count(None, 1),
        Err(BodyError::WrongFdCount { actual: 1 })
    ));
}

#[test]
fn external_body_digest_is_checked_against_bytes() {
    use carosign_broker::external_body::ExternalBody;

    let bytes = vec![9u8; 4096];
    let body = ExternalBody::for_bytes(&bytes).expect("тело");
    assert!(body.check_bytes(&bytes).is_ok());

    let mut changed = bytes;
    changed[0] = 0;
    assert!(body.check_bytes(&changed).is_err());
}
