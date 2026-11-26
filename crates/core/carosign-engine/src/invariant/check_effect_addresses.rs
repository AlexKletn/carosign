use crate::{decision::TransitionError, transition::OperationState};

/// Addresses of expected effects should be unique and already exist.
pub(crate) fn check_effect_addresses(state: &OperationState) -> Result<(), TransitionError> {
    let effects = &state.common.pending_effects;
    for (index, effect) in effects.iter().enumerate() {
        if effect.transition.value() == 0
            || effect.transition > state.common.transition_index
            || effects[..index].iter().any(|previous| {
                previous.transition == effect.transition && previous.position == effect.position
            })
        {
            return Err(TransitionError::NotSpecified {
                what: "неверный или повторный адрес ожидающего эффекта",
            });
        }
    }
    Ok(())
}
