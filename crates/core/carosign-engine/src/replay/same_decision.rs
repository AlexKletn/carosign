use crate::input::DomainInput;

use super::rebuild;

/// Do two repetitions of the same journal coincide?
///
/// E-07 test: The same inputs give the same solution and the same effects.
pub fn same_decision(inputs: &[DomainInput]) -> bool {
    let first = rebuild(
        crate::machines::verification::tests_support::initial(),
        inputs.to_vec(),
    );
    let second = rebuild(
        crate::machines::verification::tests_support::initial(),
        inputs.to_vec(),
    );
    match (first, second) {
        (Ok(a), Ok(b)) => a == b,
        (Err(a), Err(b)) => a == b,
        _ => false,
    }
}
