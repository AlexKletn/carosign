use crate::transition::OperationState;

/// Can the changing command be repeated after the reboot?
///
/// ⚠️ The answer is always `false`The question is asked only so that the rule can be seen in
/// its place and cannot be circumvented.
pub fn may_replay_mutating(state: &OperationState) -> bool {
    let _ = state.common.phase.is_armed();
    false
}
