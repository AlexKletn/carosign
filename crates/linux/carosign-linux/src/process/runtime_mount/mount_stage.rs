#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MountStage {
    TargetName,
    CreateDirectory,
    OpenDirectory,
    InspectSource,
    CreateTarget,
    CloneSource,
    RestrictSource,
    Attach,
    RestrictDirectory,
}
