#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RootStage {
    OpenFilesystem,
    SetSize,
    SetMode,
    CreateFilesystem,
    CreateMount,
    AssignOwner,
    RestrictMode,
}
