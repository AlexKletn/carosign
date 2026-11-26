#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FileRole {
    Policy,
    Manifest,
    Profile,
    Executable,
    Runtime,
}
