/// Who keeps the pattern.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Custody {
    /// Provider or secure module.
    Provider,
    /// Optional platform storage.
    PlatformVault,
}
