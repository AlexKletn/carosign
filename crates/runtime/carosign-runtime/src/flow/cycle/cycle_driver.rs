use super::CycleStep;
use crate::operation::actor::OperationActor;
use crate::operation::actor::OperationMsg;
use crate::operation::actor::Step;

/// He takes one step of the actor and says it worked.
///
/// `EffectRunner` executes effects. This loop advances the actor and reports its phase.
pub fn step_actor(actor: &mut OperationActor, message: OperationMsg) -> CycleStep {
    // ⚠️ First **lay **, then **take **: in the mail could lie observations from the previous
    // steps (service puts the answer ports between steps), and the analysis from the
    // beginning of the queue would make a step someone else's message, and `Progress` I left
    // it for later. It was like, "It's worth the surgery, but it's all right."
    if let Err(error) = actor.post(message) {
        return CycleStep::Refused(error.to_string());
    }
    let step = actor.run_once();
    match step {
        Step::Advanced { state, effects } => CycleStep::Progressed {
            phase: format!("{:?}", state.common.phase),
            effects: effects.len(),
        },
        Step::Idle => CycleStep::Waiting,
        Step::Terminal => CycleStep::Done,
        Step::Overflow => CycleStep::Refused("очередь управления переполнена".to_string()),
        // ⚠️ Refusing to log in is your word, not your wait. Turning down refusal to wait
        // would be saying nothing happens when the engine said, "You can't do that": you
        // won't look there (the step rule).
        Step::Rejected(error) => CycleStep::Refused(error.to_string()),
    }
}

/// Operation number of the cycle.
pub fn operation_of(actor: &OperationActor) -> carosign_core::ids::OperationId {
    actor.state().common.context.operation_id
}
