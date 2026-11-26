/// Storage of an existing ligament.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindingKind {
    /// Non-transparent link provider.
    ProviderOpaque,
    /// Access through restricted storage.
    ScopedVault,
}
