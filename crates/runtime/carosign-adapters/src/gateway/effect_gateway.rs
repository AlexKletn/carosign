use super::{GatewayStep, ObservedGatewayStep};

use core::fmt;

use carosign_engine::{EffectReceipt, EffectSpec, PlannedEffect};
use carosign_runtime::OperationActor;
use carosign_runtime::operation::actor::Step;

use carosign_runtime::effect_runner::{EffectRunner, Execution};

/// A gateway to one operation.
pub struct Gateway<'a> {
    runner: EffectRunner<'a>,
    sent_mutation: bool,
    operation: Option<carosign_core::ids::OperationId>,
}

impl<'a> fmt::Debug for Gateway<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Gateway(..)")
    }
}

impl<'a> Gateway<'a> {
    /// The gateway is over the performer.
    pub fn new(runner: EffectRunner<'a>) -> Self {
        Self {
            runner,
            sent_mutation: false,
            operation: None,
        }
    }

    /// Takes one step.
    ///
    /// ⚠️ Observations are returned to the actor before they return from the function: if the
    /// actor does not accept them, it is seen here and now.
    pub async fn step(&mut self, actor: &mut OperationActor) -> GatewayStep {
        if !self.bind(actor.state().common.context.operation_id) {
            return GatewayStep::Refused("шлюз принадлежит другой операции");
        }
        let step = actor.run_once();
        self.execute_step(actor, step).await
    }

    /// Performs a step with the publication of the non-terminal phase through the operation
    /// hub.
    ///
    /// The event error is returned separately and does not cancel the effects.
    pub async fn step_observed(
        &mut self,
        actor: &mut OperationActor,
        hub: &mut carosign_runtime::operation::hub::EventHub,
    ) -> Result<ObservedGatewayStep, carosign_runtime::operation::hub::AttachError> {
        let operation = actor.state().common.context.operation_id;
        if hub.operation() != operation || !self.bind(operation) {
            return Err(carosign_runtime::operation::hub::AttachError::WrongOperation);
        }
        let observed = hub.step_actor(actor)?;
        let step = self.execute_step(actor, observed.step).await;
        Ok(ObservedGatewayStep {
            step,
            notification: observed.notification,
        })
    }

    fn bind(&mut self, operation: carosign_core::ids::OperationId) -> bool {
        match self.operation {
            Some(bound) => bound == operation,
            None => {
                self.operation = Some(operation);
                true
            }
        }
    }

    async fn execute_step(&mut self, actor: &mut OperationActor, step: Step) -> GatewayStep {
        let operation_id = actor.state().common.context.operation_id;
        match step {
            Step::Terminal => {
                if actor.state().is_mutation() {
                    // There’s no reading of a steady result yet; an empty list of effects by
                    // itself doesn’t confirm anything.
                    GatewayStep::Refused("терминальный результат не подтверждён хранилищем")
                } else {
                    GatewayStep::Terminal
                }
            }
            Step::Idle => GatewayStep::Idle,
            Step::Overflow => GatewayStep::Refused("переполнение очереди управления"),
            Step::Rejected(_) => GatewayStep::Refused("вход отвергнут движком"),
            Step::Advanced { effects, .. } => {
                if effects.is_empty() {
                    return GatewayStep::Idle;
                }
                let mut executed = 0usize;
                let mut receipts = 0usize;
                let mut cancel_unconfirmed = false;
                for effect in effects {
                    if effect.spec == EffectSpec::SendMutatingCommand {
                        if self.sent_mutation {
                            // ⚠️ The second time is not: the outcome of the first is unknown.
                            return GatewayStep::Refused("повтор изменяющей отправки");
                        }
                        self.sent_mutation = true;
                    }
                    let execution = self.runner.execute(effect, operation_id).await;
                    executed += 1;
                    if matches!(execution, Execution::StorageUnavailable) {
                        if actor.state().is_mutation()
                            && actor
                                .post(carosign_runtime::OperationMsg::Domain(
                                    carosign_engine::DomainInput::StorageUnavailable,
                                ))
                                .is_err()
                        {
                            return GatewayStep::Refused("очередь наблюдений переполнена");
                        }
                        return GatewayStep::StorageUnavailable;
                    }
                    if matches!(execution, Execution::Noop | Execution::CancelUnconfirmed(_))
                        && effect.spec == EffectSpec::RequestCancel
                    {
                        // Unconfirmed cancellation should not prevent a steady record of the
                        // uncertainty of the mutation already sent.
                        cancel_unconfirmed = true;
                        continue;
                    }
                    if matches!(execution, Execution::Noop | Execution::NoDispatchPlan) {
                        return GatewayStep::Refused("эффект не исполнен");
                    }
                    if let Some(receipt) = self.runner.receipt_for(effect.spec, &execution) {
                        if self.feed(actor, effect, receipt).is_err() {
                            return GatewayStep::Refused("очередь наблюдений переполнена");
                        }
                        receipts += 1;
                    }
                }
                if cancel_unconfirmed {
                    return GatewayStep::Refused("отмена не подтверждена");
                }
                GatewayStep::Progressed {
                    effects: executed,
                    receipts,
                }
            }
        }
    }

    /// Returns the observation to the actor.
    ///
    /// ⚠️ Observation belongs to the **effect of the queue*, not to the “just fact”:
    /// therefore, it includes the place in the transition and the position itself.
    fn feed(
        &self,
        actor: &mut OperationActor,
        effect: PlannedEffect,
        receipt: EffectReceipt,
    ) -> Result<(), ()> {
        actor
            .post(carosign_runtime::OperationMsg::Domain(
                carosign_engine::DomainInput::Receipt { effect, receipt },
            ))
            .map_err(|_| ())
    }
}
