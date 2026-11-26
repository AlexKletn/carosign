use crate::commands::Command;

/// Checks that the command is allowed without authority at all.
///
/// ⚠️ Just reading and "starting the operation": the operation itself will still go through
/// the source of authority. There is no permission here and there can be no.
pub fn is_read_only(command: &Command) -> bool {
    matches!(
        command,
        Command::ListAuthenticators
            | Command::ListEnrollments { .. }
            | Command::Health
            | Command::Watch { .. }
            | Command::Status { .. }
    )
}
