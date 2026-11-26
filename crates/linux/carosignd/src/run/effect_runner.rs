use super::{
    AuthorizationBoundary, EffectReceipt, EffectRunner, EffectSpec, Execution, Observation,
    ObservedExecution, OperationActor, OperationPhase, PolicyAuthority, RunOutcome, STEPS_MAX,
    Step, ValidatedGrant, grant_matches_operation, publish_observed_mutation_terminal,
};

/// Drives the operation to the terminal or until waiting.
///
/// ⚠️ Returns ** the last result ** if it has occurred: the terminal phase without an outcome
/// is an engine error, and it is visible as `StepLimit`.
pub async fn run(
    actor: &mut OperationActor,
    runner: &mut EffectRunner<'_>,
    authority: &dyn PolicyAuthority,
    grant: &ValidatedGrant,
    observe: &mut Observation<'_>,
) -> RunOutcome {
    run_inner(actor, runner, authority, grant, observe, None).await
}

pub(super) async fn run_inner(
    actor: &mut OperationActor,
    runner: &mut EffectRunner<'_>,
    authority: &dyn PolicyAuthority,
    grant: &ValidatedGrant,
    observe: &mut Observation<'_>,
    mut events: Option<ObservedExecution<'_>>,
) -> RunOutcome {
    if !grant_matches_operation(actor, grant) {
        return RunOutcome::Refused {
            phase: actor.state().common.phase,
            reason: "разрешение не соответствует операции".to_string(),
        };
    }
    for _ in 0..STEPS_MAX {
        let step = if let Some(events) = events.as_mut() {
            match events.hub.step_actor(actor) {
                Ok(observed) => {
                    (events.notify)(observed.notification);
                    observed.step
                }
                Err(_) => {
                    return RunOutcome::Refused {
                        phase: actor.state().common.phase,
                        reason: "поток принадлежит другой операции".to_string(),
                    };
                }
            }
        } else {
            actor.run_once()
        };
        match step {
            // ⚠️ Engine failure is not waiting: your word and search, too.
            Step::Rejected(error) => {
                return RunOutcome::Refused {
                    phase: actor.state().common.phase,
                    reason: error.to_string(),
                };
            }
            Step::Overflow => {
                return RunOutcome::Refused {
                    phase: actor.state().common.phase,
                    reason: "очередь управления переполнена".to_string(),
                };
            }
            // ⚠️ The mail is empty, and the operation is waiting for an external observation:
            // a response from the provider. The steps here will not move anything further,
            // and this is "waiting," not "limiting steps": the observer waits, not the cycle.
            Step::Idle if !actor.state().common.phase.is_terminal() => {
                return RunOutcome::Waiting {
                    phase: actor.state().common.phase,
                };
            }
            Step::Idle | Step::Terminal => {
                if actor.state().is_mutation() {
                    if let Some(events) = events.as_mut()
                        && let Some(storage) = events.terminal_storage
                    {
                        return match publish_observed_mutation_terminal(
                            actor, events.hub, authority, grant, storage,
                        )
                        .await
                        {
                            Ok(publication) => {
                                (events.notify)(Ok(publication.disconnected));
                                match publication.owner_outcome {
                                    Ok(outcome) => RunOutcome::Terminal(outcome),
                                    Err(_) => RunOutcome::Refused {
                                        phase: actor.state().common.phase,
                                        reason: "полномочия владельца отозваны".to_string(),
                                    },
                                }
                            }
                            Err(error) => RunOutcome::Refused {
                                phase: actor.state().common.phase,
                                reason: format!("устойчивый итог недоступен: {error:?}"),
                            },
                        };
                    }
                    // The absence of a waiting effect does not prove the record.
                    // This cycle is not yet connected to a verified v3 reading.
                    return RunOutcome::Refused {
                        phase: actor.state().common.phase,
                        reason: "терминальный результат не подтверждён хранилищем".to_string(),
                    };
                }
                return match actor.state().common.terminal {
                    Some(outcome) => RunOutcome::Terminal(outcome),
                    None => RunOutcome::StepLimit {
                        phase: actor.state().common.phase,
                    },
                };
            }
            Step::Advanced { effects, .. } => {
                // We perform only what is given by this transition: the expected effect can
                // already be executed and it cannot be sent again.
                if effects.is_empty() {
                    continue;
                }
                let operation = actor.state().common.context.operation_id;
                for effect in effects {
                    if effect.spec == EffectSpec::RecordTerminal && !actor.state().is_mutation() {
                        // Common. MutationPort It doesn't bear the outcome of the inspection,
                        // it doesn't bear the certificate of the cleaning, it doesn't confirm
                        // the recording of the test terminal, it stays with the actor until
                        // it's handed over to a specialized owner.
                        return RunOutcome::Waiting {
                            phase: actor.state().common.phase,
                        };
                    }
                    if effect.spec == EffectSpec::SendVerify {
                        let common = &actor.state().common;
                        let (Some(worker), Some(command)) = (common.worker, common.command) else {
                            // The transmission to the session owner has not yet been
                            // prepared; the published effect remains in the actor's state.
                            return RunOutcome::Waiting {
                                phase: common.phase,
                            };
                        };
                        if worker.device_epoch != common.context.authenticator.device_epoch
                            || command.transition_index != effect.transition
                        {
                            return RunOutcome::Refused {
                                phase: common.phase,
                                reason: "контекст отправки не соответствует операции".into(),
                            };
                        }
                    }
                    // ⚠️ The effects without execution on this layer are confirmed by the
                    // service. No observation, no confirmation: the step waits, not thinks.
                    let receipt = if effect.spec == EffectSpec::RevalidateGrant {
                        if !grant_matches_operation(actor, grant) {
                            return RunOutcome::Refused {
                                phase: actor.state().common.phase,
                                reason: "разрешение не соответствует операции".to_string(),
                            };
                        }
                        Some(
                            if authority
                                .revalidate(grant, AuthorizationBoundary::BeforeDispatch)
                                .await
                                .is_ok()
                            {
                                EffectReceipt::GrantValid
                            } else {
                                EffectReceipt::GrantRevoked
                            },
                        )
                    } else if effect.spec == EffectSpec::AcquireResources {
                        observe(effect, effect.spec)
                    } else {
                        None
                    };
                    if let Some(receipt) = receipt {
                        if !matches!(
                            (effect.spec, receipt),
                            (
                                EffectSpec::AcquireResources,
                                EffectReceipt::LeaseGranted | EffectReceipt::LeaseDenied
                            ) | (
                                EffectSpec::RevalidateGrant,
                                EffectReceipt::GrantValid | EffectReceipt::GrantRevoked
                            )
                        ) {
                            return RunOutcome::Refused {
                                phase: actor.state().common.phase,
                                reason: "наблюдение не соответствует эффекту".to_string(),
                            };
                        }
                        if let Err(error) = actor.post(carosign_runtime::OperationMsg::Domain(
                            carosign_engine::DomainInput::Receipt { effect, receipt },
                        )) {
                            return RunOutcome::Refused {
                                phase: actor.state().common.phase,
                                reason: error.to_string(),
                            };
                        }
                        continue;
                    }
                    let execution = runner.execute(effect, operation).await;
                    if matches!(
                        execution,
                        Execution::StorageUnavailable
                            | Execution::NoDispatchPlan
                            | Execution::Noop
                            | Execution::CancelUnconfirmed(_)
                    ) {
                        return RunOutcome::Refused {
                            phase: actor.state().common.phase,
                            reason: match execution {
                                Execution::StorageUnavailable => "хранилище недоступно",
                                Execution::NoDispatchPlan => "нет плана отправки",
                                _ => "эффект не исполнен",
                            }
                            .to_string(),
                        };
                    }
                    if let Some(receipt) = runner.receipt_for(effect.spec, &execution)
                        && let Err(error) = actor.post(carosign_runtime::OperationMsg::Domain(
                            carosign_engine::DomainInput::Receipt { effect, receipt },
                        ))
                    {
                        return RunOutcome::Refused {
                            phase: actor.state().common.phase,
                            reason: error.to_string(),
                        };
                    }
                }
            }
        }
    }
    RunOutcome::StepLimit {
        phase: actor.state().common.phase,
    }
}

/// Whether the operation is waiting for external surveillance.
pub fn waits_for_provider(phase: OperationPhase) -> bool {
    matches!(
        phase,
        OperationPhase::Running | OperationPhase::InFlight | OperationPhase::EvaluatingEvidence
    )
}

/// How many steps have to be taken to get there.
pub fn steps_to_send(actor: &OperationActor) -> Option<usize> {
    let phase = actor.state().common.phase;
    if waits_for_provider(phase) {
        return None;
    }
    Some(match phase {
        OperationPhase::New => 3,
        OperationPhase::WaitingLease => 2,
        OperationPhase::ReadyToDispatch => 1,
        _ => 0,
    })
}

/// The effect that the operation is waiting for is next.
pub fn next_effect(actor: &OperationActor) -> Option<EffectSpec> {
    actor.pending().first().map(|effect| effect.spec)
}
