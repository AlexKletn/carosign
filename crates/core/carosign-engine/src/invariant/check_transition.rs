use crate::{
    decision::TransitionError,
    input::{EffectSpec, PlannedEffect},
    transition::{OperationPhase, OperationState},
};

/// Has the transition allowed violation of invariants?
pub fn check(next: &OperationState, effects: &[PlannedEffect]) -> Result<(), TransitionError> {
    // (1) Modifying shipment is permitted only after `DispatchArmed` She's already down on
    // the disc. In one transition, she's just out of `Arming`.
    let sends_mutation = effects
        .iter()
        .any(|effect| effect.spec == EffectSpec::SendMutatingCommand);
    if sends_mutation && !matches!(next.common.phase, OperationPhase::InFlight) {
        return Err(TransitionError::NotSpecified {
            what: "изменяющая отправка вне фазы InFlight",
        });
    }

    // 2. `DispatchArmed` There is no one to send; there must be a confirmation from the disk
    // between them.
    let arms = effects
        .iter()
        .any(|effect| effect.spec == EffectSpec::PersistDispatchArmed);
    if arms && sends_mutation {
        return Err(TransitionError::NotSpecified {
            what: "DispatchArmed и изменяющая отправка в одном переходе",
        });
    }

    // 3 The evidence is accepted only from the verification phase.
    let accepts_evidence = effects
        .iter()
        .any(|effect| effect.spec == EffectSpec::VerifyEvidence);
    if accepts_evidence
        && !matches!(
            next.common.phase,
            OperationPhase::EvaluatingEvidence | OperationPhase::Running
        )
    {
        return Err(TransitionError::NotSpecified {
            what: "проверка доказательства вне своей фазы",
        });
    }

    // 4 The terminal phase must have an outcome.
    if next.common.phase.is_terminal() && next.common.terminal.is_none() {
        return Err(TransitionError::NotSpecified {
            what: "терминального состояния нет исхода",
        });
    }

    // The proof cannot be left tied to the completed transaction; the recording of the
    // terminal must not begin until the confirmation of the withdrawal.
    if next
        .body
        .verification()
        .is_some_and(|verification| verification.evidence.is_some())
        && (next.common.phase.is_terminal()
            || effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::RecordTerminal)
            || next
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::RecordTerminal))
    {
        return Err(TransitionError::NotSpecified {
            what: "терминал до уничтожения удержанного доказательства",
        });
    }

    Ok(())
}
