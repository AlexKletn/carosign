/// The stage of preparation of namespaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NamespaceStage {
    /// Creation of independent mounting spaces and network.
    Create,
    /// Disable the distribution of mounts in both directions.
    PrivatePropagation,
}
