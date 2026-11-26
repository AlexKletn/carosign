/// The result of the quarantine installation is not a launch permit.
#[derive(Debug)]
pub enum QuarantineCommandError {
    /// Wrong arguments, authority, or the environment of an assistant.
    Refused,
    /// Preparation or fixation has not been confirmed.
    Unavailable,
    /// The recording is in quarantine and requires independent restoration.
    RecoveryRequired,
}
