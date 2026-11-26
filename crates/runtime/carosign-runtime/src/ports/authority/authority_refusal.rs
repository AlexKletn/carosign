/// Renunciation of source of authority.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuthorityRefusal {
    /// Politics forbids.
    Denied,
    /// Permission revoked.
    Revoked,
    /// Time's up.
    Expired,
    /// The source is unavailable - also a denial.
    Unavailable,
}
