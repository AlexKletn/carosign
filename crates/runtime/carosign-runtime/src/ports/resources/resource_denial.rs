/// Denial of resources.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResourceDenial {
    /// Occupied by others.
    Busy,
    /// Politics forbids.
    Denied,
    /// The source is unavailable.
    Unavailable,
}
