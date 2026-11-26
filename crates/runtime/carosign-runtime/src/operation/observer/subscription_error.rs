use crate::ports::AuthorityRefusal;

/// Error of subscribing or re-checking authority.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubscriptionError {
    /// The source of authority has denied or revoked the right.
    Authority(AuthorityRefusal),
    /// Subscription cannot be added to this stream.
    Attach(super::super::hub::AttachError),
    /// You need a stable result before issuing a terminal event.
    TerminalNeedsStorage,
    /// The subscription has already been deleted or belongs to another stream.
    Detached,
    /// A sustainable outcome is not available or available.
    TerminalUnavailable,
}
