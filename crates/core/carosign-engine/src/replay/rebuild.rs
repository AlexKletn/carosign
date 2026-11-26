use crate::{decision::TransitionError, input::DomainInput, transition::OperationState};

/// Restores state by filing recorded inputs in order.
///
/// ⚠️ The error in the log is not "repaired" by a line omission: the repeat stops at the
/// first uncoordinated input.
pub fn rebuild(
    initial: OperationState,
    inputs: impl IntoIterator<Item = DomainInput>,
) -> Result<OperationState, TransitionError> {
    crate::invariant::check_scope(&initial)?;
    crate::invariant::check_reconciliation(&initial)?;
    crate::invariant::check_phase(initial.kind(), initial.common.phase)?;
    crate::invariant::check_effect_addresses(&initial)?;
    crate::invariant::check(&initial, &[])?;
    let mut state = initial;
    for input in inputs {
        state = crate::decide(&state, input)?.next;
    }
    Ok(state)
}
