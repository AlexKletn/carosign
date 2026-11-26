/// Where does the provider get the template?
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BindingKind {
    /// The provider stores and disassembles the link itself.
    ProviderOpaque,
    /// A couple of templates through the authorized vault.
    ScopedVault,
}
