use crate::{input::EffectSpec, transition::OperationState};

pub(super) fn destroy_held_if_needed(state: &OperationState, effects: &mut Vec<EffectSpec>) {
    if state
        .body
        .verification()
        .is_some_and(|verification| verification.evidence.is_some())
        && !state
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::DestroyEvidence)
    {
        effects.push(EffectSpec::DestroyEvidence);
    }
}
