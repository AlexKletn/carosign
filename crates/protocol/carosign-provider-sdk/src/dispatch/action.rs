use crate::command::Command;

/// What the worker should do with the command.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Accept and work.
    Run,
    /// Refuse immediately.
    Refuse,
}

/// Whether the worker can take the command.
///
/// ⚠️ An out-of-date generation (the wrong era) is a failure to act: otherwise the command of
/// the last session will reach the current one.
pub fn admittance(command: &Command, bound_epoch: u64, command_epoch: u64) -> Action {
    if matches!(command, Command::Shutdown) {
        return Action::Run;
    }
    if bound_epoch == 0 || bound_epoch != command_epoch {
        return Action::Refuse;
    }
    let authenticator = match command {
        Command::PrepareEnrollmentChallenge { authenticator, .. }
        | Command::StartVerification { authenticator, .. }
        | Command::StartEnrollment { authenticator, .. }
        | Command::StartRemoval { authenticator, .. }
        | Command::QueryMutation { authenticator, .. }
        | Command::Describe { authenticator }
        | Command::Quiesce { authenticator }
        | Command::CloseAuthenticator { authenticator } => Some(authenticator),
        Command::ContinueEnrollment { enrollment } => {
            return admittance(enrollment, bound_epoch, command_epoch);
        }
        Command::RequestCancel { .. }
        | Command::EnrollmentTransaction { .. }
        | Command::Shutdown => None,
    };
    if authenticator.is_some_and(|reference| {
        reference.device_epoch != command_epoch
            || carosign_protocol::validation::require_id(
                &reference.authenticator_id,
                "authenticator_id",
            )
            .is_err()
    }) {
        return Action::Refuse;
    }
    Action::Run
}

#[cfg(test)]
#[path = "action.tests.rs"]
mod tests;
