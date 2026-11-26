/// Failure to install a fence or restore the actor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecoveryError {
    /// There are too many unfinished areas.
    Limit,
    /// The area or change is already occupied with another recovery.
    Conflict,
    /// There is no appropriate fencing for this actor.
    NotBlocked,
    /// The condition is not an acceptable mutation on reconciliation.
    InvalidActor,
}
