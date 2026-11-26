/// Refusal to enter one recorded session.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum CountedServeError {
    /// The settings were not checked before the connection was received.
    InvalidConfig,
    /// The connection, its identifier or place in the account is not available.
    Accept(crate::server::CountedAcceptError),
    /// It is not possible to independently detect the owner's rupture.
    WatchUnavailable,
}
