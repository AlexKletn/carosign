use super::*;

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArmAttemptFailure {
    AlreadyAttempted,
    Capacity,
    Durable(DurableArmFailure),
}
