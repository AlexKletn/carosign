/// Initial choice stored as data, without identity assertion.
#[derive(Clone, PartialEq, Eq)]
pub enum RequestedSubject {
    /// Current user.
    Current,
    /// Account name tip.
    AccountHint(String),
    /// Non-transparent data source of personalities.
    Opaque(Vec<u8>),
}
