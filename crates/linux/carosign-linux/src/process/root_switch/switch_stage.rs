#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SwitchStage {
    CheckRoot,
    CheckDescriptors,
    Namespace,
    ChangeDirectory,
    ChangeRoot,
    LeaveOldDirectory,
    CloseDescriptors,
}
