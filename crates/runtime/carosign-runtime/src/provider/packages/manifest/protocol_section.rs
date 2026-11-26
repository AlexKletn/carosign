/// Section `[protocol]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolSection {
    pub major: u32,
    /// The youngest.
    pub min_minor: u32,
    /// Younger before.
    pub max_minor: u32,
}
