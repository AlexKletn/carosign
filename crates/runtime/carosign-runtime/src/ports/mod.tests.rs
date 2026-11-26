use super::*;
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::OperationId;
use carosign_core::ids::{
    AuthenticatorId, CommandId, EnrollmentId, LeaseId, MutationId, WorkerInstanceId,
};
use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};

fn request() -> MutationArmRequest {
    let mutation = MutationId::from_bytes([1; 16]).unwrap();
    let command = CommandId::from_bytes([2; 16]).unwrap();
    let subject = carosign_core::identity::SubjectRef {
        authority: carosign_core::ids::AuthorityId::from_bytes([5; 16]).unwrap(),
        subject: carosign_core::ids::SubjectId::from_bytes([6; 16]).unwrap(),
    };
    let planned_new = EnrollmentRef {
        id: EnrollmentId::from_bytes([7; 16]).unwrap(),
        authenticator: AuthenticatorId::from_bytes([8; 16]).unwrap(),
        generation: EnrollmentGeneration::FIRST,
    };
    let mut scope = [0; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN];
    scope[..32].copy_from_slice(&subject.to_canonical());
    scope[32..].copy_from_slice(planned_new.authenticator.as_bytes());
    MutationArmRequest {
        operation: OperationId::from_bytes([3; 16]).unwrap(),
        uid: 1000,
        effect: PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(1), 0),
        arm: carosign_persistence::journal::ArmMutation {
            mutation,
            command,
            worker: WorkerInstanceId::from_bytes([4; 16]).unwrap(),
            epoch: DeviceEpoch::FIRST,
            recovery_ref: None,
        },
        scope,
        label: Some(carosign_core::enrollment::EnrollmentLabel::new("test").unwrap()),
        lease: LeaseId::from_bytes([6; 16]).unwrap(),
        plan: CommandPlan::Mutate {
            mutation,
            command,
            authorization: EnrollmentAuthorization::None,
            change: MutationChange::Create { planned_new },
        },
        logical_plan: carosign_core::operation::plan::MutationPlan::new(
            subject,
            planned_new.authenticator,
            carosign_core::operation::plan::PlanTransition::Create { planned_new },
            Vec::new(),
            [0; 32],
        )
        .and_then(|plan| {
            plan.with_label(Some(
                carosign_core::enrollment::EnrollmentLabel::new("test").unwrap(),
            ))
        })
        .unwrap(),
    }
}

#[test]
fn arm_request_refuses_foreign_effect_mutation_and_command() {
    let original = request();
    assert!(original.is_consistent());
    let mut foreign = original.clone();
    foreign.effect = PlannedEffect::at(EffectSpec::SendVerify, TransitionIndex::new(1), 0);
    assert!(!foreign.is_consistent());
    let mut foreign = original.clone();
    foreign.arm.mutation = MutationId::from_bytes([9; 16]).unwrap();
    assert!(!foreign.is_consistent());
    let mut foreign = original;
    foreign.arm.command = CommandId::from_bytes([9; 16]).unwrap();
    assert!(!foreign.is_consistent());
}

#[test]
fn arm_request_requires_scope_and_new_target_from_logical_plan() {
    let original = request();
    let mut foreign = original.clone();
    foreign.scope[0] ^= 1;
    assert!(!foreign.is_consistent());
    let mut foreign = original;
    let CommandPlan::Mutate {
        change: MutationChange::Create { planned_new },
        ..
    } = &mut foreign.plan
    else {
        unreachable!()
    };
    planned_new.id = EnrollmentId::from_bytes([91; 16]).unwrap();
    assert!(!foreign.is_consistent());
}

#[test]
fn arm_request_rejects_altered_remove_binding_and_transition() {
    use crate::operation::bindings::{BindingKind, EnrollmentBinding};
    use carosign_core::operation::plan::{
        BindingKind as StoredKind, BindingSnapshot, MutationPlan, PlanTransition,
    };
    let mut request = request();
    let subject = request.logical_plan.subject();
    let authenticator = request.logical_plan.authenticator();
    let old = EnrollmentRef {
        id: EnrollmentId::from_bytes([42; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let binding = EnrollmentBinding {
        enrollment: old,
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![1, 2, 3],
        format_id: Some("format".into()),
        format_version: 2,
    };
    request.logical_plan = MutationPlan::new(
        subject,
        authenticator,
        PlanTransition::Remove { old },
        vec![BindingSnapshot {
            enrollment: old,
            subject,
            kind: StoredKind::ProviderOpaque,
            provider_ref_digest: carosign_persistence::mutation_plan::sha256(
                &binding.provider_ref,
            ),
            format_id: "format".into(),
            format_version: 2,
        }],
        [0; 32],
    )
    .unwrap();
    request.label = None;
    let CommandPlan::Mutate { change, .. } = &mut request.plan else {
        unreachable!()
    };
    *change = MutationChange::Removal {
        enrollment: old,
        binding,
    };
    assert!(request.is_consistent());
    for change in 0..4 {
        let mut foreign = request.clone();
        let CommandPlan::Mutate {
            change: MutationChange::Removal { binding, .. },
            ..
        } = &mut foreign.plan
        else {
            unreachable!()
        };
        match change {
            0 => binding.provider_ref.push(4),
            1 => binding.kind = BindingKind::ScopedVault,
            2 => binding.format_id = Some("other".into()),
            _ => binding.format_version += 1,
        }
        assert!(!foreign.is_consistent());
    }
    let planned_new = EnrollmentRef {
        id: EnrollmentId::from_bytes([43; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    request.logical_plan = MutationPlan::new(
        subject,
        authenticator,
        PlanTransition::Replace { old, planned_new },
        request.logical_plan.pre_bindings().to_vec(),
        [0; 32],
    )
    .and_then(|plan| {
        plan.with_label(Some(
            carosign_core::enrollment::EnrollmentLabel::new("test").unwrap(),
        ))
    })
    .unwrap();
    request.label = Some(carosign_core::enrollment::EnrollmentLabel::new("test").unwrap());
    let CommandPlan::Mutate {
        change: MutationChange::Removal { binding, .. },
        ..
    } = &request.plan
    else {
        unreachable!()
    };
    let binding = binding.clone();
    let CommandPlan::Mutate { change, .. } = &mut request.plan else {
        unreachable!()
    };
    *change = MutationChange::Replace {
        planned_new,
        old: binding,
    };
    assert!(request.is_consistent());
    let CommandPlan::Mutate {
        change: MutationChange::Replace { planned_new, .. },
        ..
    } = &mut request.plan
    else {
        unreachable!()
    };
    planned_new.generation = EnrollmentGeneration::new(2).unwrap();
    assert!(!request.is_consistent());
}
