use core::fmt;

/// Session error.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// The provider didn't take the command.
    NotAccepted,
    /// Second score.
    SecondTerminal,
    /// The event refers to another worker, device, or operation.
    StaleProgress,
    /// The event belongs to another command.
    WrongCommand,
    /// The move came after the end.
    ProgressAfterTerminal,
    /// The move number does not start the sequence or grow.
    ProgressOutOfOrder,
    /// The event is not a move or contains an incorrect identifier.
    MalformedProgress,
    /// The outcome of the change is incorrect or does not match the plan of the operation.
    InvalidMutationOutcome,
    /// The outcome of the check is incorrect or does not apply to the authorized set.
    InvalidVerificationOutcome,
}

impl fmt::Debug for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotAccepted => "SessionError::NotAccepted",
            Self::SecondTerminal => "SessionError::SecondTerminal",
            Self::StaleProgress => "SessionError::StaleProgress",
            Self::WrongCommand => "SessionError::WrongCommand",
            Self::ProgressAfterTerminal => "SessionError::ProgressAfterTerminal",
            Self::ProgressOutOfOrder => "SessionError::ProgressOutOfOrder",
            Self::MalformedProgress => "SessionError::MalformedProgress",
            Self::InvalidMutationOutcome => "SessionError::InvalidMutationOutcome",
            Self::InvalidVerificationOutcome => "SessionError::InvalidVerificationOutcome",
        })
    }
}

impl SessionError {}
