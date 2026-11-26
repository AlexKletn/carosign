/// Cleaning Stage: An indefinite call does not repeat automatically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CleanupStage {
    NotStarted,
    Quiescing,
    Releasing,
    Uncertain,
    Released,
}
