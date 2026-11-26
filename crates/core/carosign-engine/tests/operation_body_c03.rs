//! Type of operation - a variant of the state body (solution C-03).
//!
//! ⚠️ Verification cannot physically carry the state of change, and registration cannot
//! depict verification: the compiler will not give a separate feature. `kind !=
//! mutation.is_some()` It does not exist at all, so there is nothing to differ from it.

use carosign_core::enrollment::EnrollmentLabel;
use carosign_core::ids::{AuthenticatorId, EnrollmentId, MutationId};
use carosign_core::modality::OperationKind;
use carosign_core::operation::mutation::{EnrollRequest, RemoveRequest};
use carosign_engine::tests_support::initial;
use carosign_engine::{
    DomainInput, EffectReceipt, EffectSpec, OperationBody, OperationPhase, OperationState,
    PlannedEffect, TransitionError, TransitionIndex, decide,
};

fn context() -> carosign_core::operation::context::OperationContext {
    initial().common.context.clone()
}

fn subject() -> carosign_core::identity::SubjectRef {
    context().subject
}

fn enrollment() -> OperationState {
    OperationState::enrollment(
        context(),
        EnrollRequest {
            subject: subject(),
            authenticator: AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
            target: EnrollmentLabel::new("палец").expect("метка допустима"),
            replace: None,
        },
        MutationId::from_bytes([7u8; 16]).expect("валид"),
    )
}

fn removal() -> OperationState {
    OperationState::removal(
        context(),
        RemoveRequest {
            subject: subject(),
            enrollment: carosign_core::enrollment::EnrollmentRef {
                id: EnrollmentId::from_bytes([8u8; 16]).expect("валид"),
                authenticator: AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
                generation: carosign_core::enrollment::EnrollmentGeneration::FIRST,
            },
        },
        MutationId::from_bytes([9u8; 16]).expect("валид"),
    )
}

fn receipt(index: u64, spec: EffectSpec, receipt: EffectReceipt) -> DomainInput {
    DomainInput::Receipt {
        effect: PlannedEffect::at(spec, TransitionIndex::new(index), 0),
        receipt,
    }
}

#[test]
fn mutation_dispatch_binding_predicts_exact_durable_send_for_both_machines() {
    use carosign_core::ids::{CommandId, LeaseId, WorkerInstanceId};
    use carosign_engine::WorkerRef;
    for mut state in [enrollment(), removal()] {
        state.common.phase = OperationPhase::Arming;
        state.common.transition_index = TransitionIndex::new(10);
        let arm = PlannedEffect::at(
            EffectSpec::PersistDispatchArmed,
            TransitionIndex::new(10),
            0,
        );
        state.common.pending_effects = vec![arm];
        assert!(matches!(
            decide(
                &state,
                DomainInput::Receipt {
                    effect: arm,
                    receipt: EffectReceipt::DispatchArmedDurable
                }
            ),
            Err(TransitionError::ReceiptMismatch)
        ));
        let lease = LeaseId::from_bytes([21; 16]).unwrap();
        let worker = WorkerRef {
            instance: WorkerInstanceId::from_bytes([22; 16]).unwrap(),
            device_epoch: state.common.context.authenticator.device_epoch,
        };
        let command = CommandId::from_bytes([23; 16]).unwrap();
        let bind = DomainInput::BindMutationDispatch {
            effect: arm,
            lease,
            worker,
            command,
        };
        let bound = decide(&state, bind.clone()).unwrap();
        assert_eq!(bound.next.common.phase, OperationPhase::Arming);
        assert!(bound.effects.is_empty());
        assert_eq!(bound.next.common.lease, Some(lease));
        assert_eq!(bound.next.common.worker, Some(worker));
        let predicted = bound.next.common.command.unwrap();
        assert_eq!(predicted.id, command);
        assert_eq!(predicted.transition_index, TransitionIndex::new(12));
        assert!(decide(&bound.next, bind).is_err());
        let armed = decide(
            &bound.next,
            DomainInput::Receipt {
                effect: arm,
                receipt: EffectReceipt::DispatchArmedDurable,
            },
        )
        .unwrap();
        assert_eq!(
            armed.effects,
            vec![PlannedEffect::at(
                EffectSpec::SendMutatingCommand,
                predicted.transition_index,
                0
            )]
        );
    }
}

#[test]
fn mutation_binding_rejects_foreign_effect_epoch_lease_and_overflow() {
    use carosign_core::device::DeviceEpoch;
    use carosign_core::ids::{CommandId, LeaseId, WorkerInstanceId};
    use carosign_engine::WorkerRef;
    let mut state = enrollment();
    state.common.phase = OperationPhase::Arming;
    state.common.transition_index = TransitionIndex::new(10);
    let arm = PlannedEffect::at(
        EffectSpec::PersistDispatchArmed,
        TransitionIndex::new(10),
        0,
    );
    state.common.pending_effects = vec![arm];
    let lease = LeaseId::from_bytes([21; 16]).unwrap();
    let worker = WorkerRef {
        instance: WorkerInstanceId::from_bytes([22; 16]).unwrap(),
        device_epoch: state.common.context.authenticator.device_epoch,
    };
    let command = CommandId::from_bytes([23; 16]).unwrap();
    let bind = |effect, lease, worker| DomainInput::BindMutationDispatch {
        effect,
        lease,
        worker,
        command,
    };
    let foreign = PlannedEffect::at(EffectSpec::PersistDispatchArmed, TransitionIndex::new(9), 0);
    assert!(decide(&state, bind(foreign, lease, worker)).is_err());
    assert!(matches!(
        decide(
            &state,
            bind(
                arm,
                lease,
                WorkerRef {
                    device_epoch: DeviceEpoch::new(2).unwrap(),
                    ..worker
                }
            )
        ),
        Err(TransitionError::StaleFencing)
    ));
    let mut other_lease = state.clone();
    other_lease.common.lease = Some(LeaseId::from_bytes([24; 16]).unwrap());
    assert!(decide(&other_lease, bind(arm, lease, worker)).is_err());
    let mut duplicate = state.clone();
    duplicate.common.pending_effects.push(PlannedEffect::at(
        EffectSpec::PersistDispatchArmed,
        TransitionIndex::new(9),
        0,
    ));
    assert!(decide(&duplicate, bind(arm, lease, worker)).is_err());
    let mut overflow = state.clone();
    overflow.common.transition_index = TransitionIndex::new(u64::MAX - 1);
    assert!(matches!(
        decide(&overflow, bind(arm, lease, worker)),
        Err(TransitionError::TransitionIndexOverflow)
    ));
    let before = state.clone();
    assert!(decide(&initial(), bind(arm, lease, worker)).is_err());
    assert_eq!(state, before);
}

#[test]
fn mutation_bound_send_cannot_use_a_stale_predicted_transition() {
    use carosign_core::ids::{CommandId, LeaseId, WorkerInstanceId};
    use carosign_engine::WorkerRef;
    let mut state = enrollment();
    state.common.phase = OperationPhase::Arming;
    state.common.transition_index = TransitionIndex::new(10);
    let arm = PlannedEffect::at(
        EffectSpec::PersistDispatchArmed,
        TransitionIndex::new(10),
        0,
    );
    state.common.pending_effects.push(arm);
    let bound = decide(
        &state,
        DomainInput::BindMutationDispatch {
            effect: arm,
            lease: LeaseId::from_bytes([21; 16]).unwrap(),
            worker: WorkerRef {
                instance: WorkerInstanceId::from_bytes([22; 16]).unwrap(),
                device_epoch: state.common.context.authenticator.device_epoch,
            },
            command: CommandId::from_bytes([23; 16]).unwrap(),
        },
    )
    .unwrap()
    .next;
    let mut shifted = bound.clone();
    shifted.common.transition_index = shifted.common.transition_index.next().unwrap();
    assert!(matches!(
        decide(
            &shifted,
            DomainInput::Receipt {
                effect: arm,
                receipt: EffectReceipt::DispatchArmedDurable,
            }
        ),
        Err(TransitionError::ReceiptMismatch)
    ));
    assert_eq!(shifted.common.phase, OperationPhase::Arming);
    assert!(shifted.common.pending_effects.contains(&arm));
}

#[test]
fn kind_is_derived_from_body_not_stored() {
    assert_eq!(initial().kind(), OperationKind::Verify);
    assert_eq!(enrollment().kind(), OperationKind::Enroll);
    assert_eq!(removal().kind(), OperationKind::Remove);
}

#[test]
fn required_grant_scope_is_derived_from_the_exact_operation() {
    use carosign_core::authorization::GrantScope;
    let verification = initial();
    assert_eq!(
        verification.required_grant_scope(),
        GrantScope::Verify {
            subject: verification.common.context.subject,
            authenticator: verification.common.context.authenticator.id,
            purpose: verification.common.context.purpose,
        }
    );
    let enrollment = enrollment();
    let OperationBody::Enrollment { request, .. } = &enrollment.body else {
        unreachable!()
    };
    assert_eq!(
        enrollment.required_grant_scope(),
        GrantScope::Enroll {
            subject: request.subject,
            authenticator: request.authenticator,
            replace: request.replace,
        }
    );
    let removal = removal();
    let OperationBody::Removal { request, .. } = &removal.body else {
        unreachable!()
    };
    assert_eq!(
        removal.required_grant_scope(),
        GrantScope::Remove {
            subject: request.subject,
            enrollment: request.enrollment,
        }
    );
}

#[test]
fn checked_enrollment_constructor_binds_the_immutable_plan() {
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
    use carosign_core::operation::plan::{MutationPlan, PlanTransition};
    let context = context();
    let planned_new = EnrollmentRef {
        id: EnrollmentId::from_bytes([21; 16]).unwrap(),
        authenticator: context.authenticator.id,
        generation: EnrollmentGeneration::FIRST,
    };
    let plan = MutationPlan::new(
        context.subject,
        context.authenticator.id,
        PlanTransition::Create { planned_new },
        vec![],
        [1; 32],
    )
    .unwrap();
    let request = EnrollRequest {
        subject: context.subject,
        authenticator: context.authenticator.id,
        target: EnrollmentLabel::new("палец").unwrap(),
        replace: None,
    };
    let mutation = MutationId::from_bytes([7; 16]).unwrap();
    let mut state = OperationState::enrollment_with_plan(
        context.clone(),
        request.clone(),
        mutation,
        plan.clone(),
    )
    .unwrap();
    assert_eq!(state.mutation().unwrap().planned_new, Some(planned_new));
    assert_eq!(state.mutation().unwrap().plan.as_ref(), Some(&plan));
    let mut foreign = request;
    foreign.replace = Some(planned_new);
    assert!(OperationState::enrollment_with_plan(context, foreign, mutation, plan).is_err());
    state.body.mutation_mut().unwrap().planned_new = None;
    assert!(carosign_engine::replay::rebuild(state.clone(), []).is_err());
    assert!(decide(&state, DomainInput::Authorized).is_err());
}

#[test]
fn verification_has_no_mutation_state() {
    let state = initial();
    assert!(state.mutation().is_none());
    assert!(!state.is_mutation());
    assert!(matches!(state.body, OperationBody::Verification(_)));
}

#[test]
fn mutations_always_hold_mutation_state() {
    for state in [enrollment(), removal()] {
        assert!(state.mutation().is_some());
        assert!(state.is_mutation());
    }
}

#[test]
fn enroll_replace_is_expressed_by_request_not_a_fourth_machine() {
    // ⚠️ Replacement is a request field, not a separate machine.
    let state = OperationState::enrollment(
        context(),
        EnrollRequest {
            subject: subject(),
            authenticator: AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
            target: EnrollmentLabel::new("замена").expect("метка допустима"),
            replace: Some(carosign_core::enrollment::EnrollmentRef {
                id: EnrollmentId::from_bytes([8u8; 16]).expect("валид"),
                authenticator: AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
                generation: carosign_core::enrollment::EnrollmentGeneration::FIRST,
            }),
        },
        MutationId::from_bytes([7u8; 16]).expect("валид"),
    );
    assert_eq!(state.kind(), OperationKind::Enroll);
    match &state.body {
        OperationBody::Enrollment { request, .. } => assert!(request.replace.is_some()),
        other => panic!("ожидалось тело регистрации, а пришло {other:?}"),
    }
}

#[test]
fn verification_send_into_mutation_is_rejected_without_side_effects() {
    // C-03: `SendVerify` c the registration machine - refusal, no state, no transitions, no
    // effects.
    let state = enrollment();
    let before = state.clone();

    for input in [
        receipt(0, EffectSpec::SendVerify, EffectReceipt::SendAccepted),
        DomainInput::VerificationSent,
        DomainInput::VerificationNotSent,
        DomainInput::VerificationOutcomeUnknown,
    ] {
        let result = decide(&state, input);
        assert!(
            matches!(
                result,
                Err(TransitionError::InvalidInputForOperation {
                    kind: OperationKind::Enroll,
                    belongs_to: OperationKind::Verify,
                })
            ),
            "ожидался отказ по виду, а пришло {result:?}"
        );
        assert_eq!(state, before, "состояние не должно меняться");
    }
}

#[test]
fn verification_chain_still_works_through_its_own_inputs() {
    let mut state = initial();
    for input in [
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        receipt(2, EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
    ] {
        state = decide(&state, input).expect("переход допустим").next;
    }
    assert_eq!(state.common.phase, OperationPhase::Running);
    state = decide(&state, DomainInput::VerificationSent)
        .expect("отправка проверки допустима")
        .next;
    assert_eq!(state.common.phase, OperationPhase::Running);
    state = decide(&state, DomainInput::VerificationNotSent)
        .expect("отказ запуска допустим")
        .next;
    assert_eq!(state.common.phase, OperationPhase::Finalizing);
}

#[test]
fn mutation_inputs_into_verification_are_rejected() {
    let state = initial();
    let result = decide(
        &state,
        receipt(
            0,
            EffectSpec::SendMutatingCommand,
            EffectReceipt::MutationNotSent,
        ),
    );
    assert!(result.is_err(), "изменяющий вход в проверке — отказ");
}

#[test]
fn incompatible_restored_phase_is_rejected_even_without_replay_inputs() {
    for mut state in [initial(), enrollment(), removal()] {
        state.common.phase = if state.kind() == OperationKind::Verify {
            OperationPhase::InFlight
        } else {
            OperationPhase::Running
        };
        let snapshot = state.clone();
        assert!(decide(&state, DomainInput::WorkerLost).is_err());
        assert!(carosign_engine::replay::rebuild(state.clone(), []).is_err());
        assert_eq!(state, snapshot);
    }
}

#[test]
fn empty_replay_rejects_terminal_state_without_outcome() {
    let mut state = initial();
    state.common.phase = OperationPhase::Terminal;
    assert!(carosign_engine::replay::rebuild(state, []).is_err());
}

#[test]
fn restored_effect_addresses_must_exist_and_be_unique() {
    let valid = decide(&initial(), DomainInput::Authorized).unwrap().next;
    assert!(carosign_engine::replay::rebuild(valid.clone(), []).is_ok());
    let effect = valid.common.pending_effects[0];
    let mut cases = Vec::new();
    let mut zero = valid.clone();
    zero.common.pending_effects[0].transition = TransitionIndex::INITIAL;
    cases.push(zero);
    let mut future = valid.clone();
    future.common.pending_effects[0].transition = valid.common.transition_index.next().unwrap();
    cases.push(future);
    let mut duplicate = valid;
    duplicate.common.pending_effects.push(PlannedEffect::at(
        EffectSpec::SendVerify,
        effect.transition,
        effect.position,
    ));
    cases.push(duplicate);
    for state in cases {
        assert!(carosign_engine::replay::rebuild(state.clone(), []).is_err());
        assert!(decide(&state, DomainInput::Progress).is_err());
    }
}

#[test]
fn restored_reconciliation_must_match_registry_action() {
    use carosign_core::enrollment::EnrollmentGeneration;
    use carosign_core::ids::EvidenceId;
    use carosign_engine::reconcile::{
        ReconciliationFailure, ReconciliationVerdict, RegistryAction, VerifiedState,
    };

    let evidence = VerifiedState {
        enrollment: EnrollmentId::from_bytes([8; 16]).unwrap(),
        generation: EnrollmentGeneration::FIRST,
        provenance: EvidenceId::from_bytes([9; 16]).unwrap(),
    };
    for (verdict, action) in [
        (ReconciliationVerdict::DesiredStateConfirmed(evidence), None),
        (
            ReconciliationVerdict::DesiredStateConfirmed(evidence),
            Some(RegistryAction::RecordNoChange),
        ),
        (
            ReconciliationVerdict::PreStateConfirmed(evidence),
            Some(RegistryAction::ApplyDesiredState),
        ),
        (
            ReconciliationVerdict::Indeterminate(ReconciliationFailure::Unknown),
            None,
        ),
    ] {
        for mut state in [enrollment(), removal()] {
            let mutation = match &mut state.body {
                OperationBody::Enrollment { state, .. } | OperationBody::Removal { state, .. } => {
                    state
                }
                OperationBody::Verification(_) => unreachable!(),
            };
            mutation.reconciliation = Some(verdict);
            mutation.registry = action;
            assert!(carosign_engine::replay::rebuild(state.clone(), []).is_err());
            assert!(decide(&state, DomainInput::WorkerLost).is_err());
        }
    }
}
