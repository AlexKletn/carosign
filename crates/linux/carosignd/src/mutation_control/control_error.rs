#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlError {
    Watch,
    Reader,
    /// The quarantine has not been stopped or sustained; repetition is prohibited.
    Uncertain,
}
