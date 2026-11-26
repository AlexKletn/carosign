/// The channel to the owner.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OwnerChannel {
    /// We got a link.
    Attached,
    /// He passed out.
    Detached,
}
