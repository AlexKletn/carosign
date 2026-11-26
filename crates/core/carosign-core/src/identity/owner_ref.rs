use crate::ids::{AuthorityId, GrantId};
use core::fmt;

/// Who owns the operation: the source of trust and the permission issued by it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnerRef {
    /// A source of trust.
    pub authority: AuthorityId,
    /// Permission under which the operation is taking place.
    pub binding: GrantId,
}

impl fmt::Debug for OwnerRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnerRef(..)")
    }
}
