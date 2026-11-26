/// What the secure module said.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProtectedAnswer {
    /// Copy that.
    Verified,
    Unavailable,
    /// It's not checked.
    Unverifiable,
}
