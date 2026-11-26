use super::ValidatedGrant;
use crate::ids::AuthorityId;
use crate::ids::GrantId;
use core::fmt;

/// Reference to permission for transfer.
///
/// The reference conveys no authority. Obtain fresh authorization from the authority source.
/// Only a confirmed grant can produce this reference.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopedGrantRef {
    id: GrantId,
    issuer: AuthorityId,
}

impl ScopedGrantRef {
    /// Reference to a confirmed permit.
    pub fn of(grant: &ValidatedGrant) -> Self {
        Self {
            id: grant.id(),
            issuer: grant.issuer(),
        }
    }

    /// Identifier referred to.
    pub fn id(self) -> GrantId {
        self.id
    }

    /// The owner to ask.
    pub fn issuer(self) -> AuthorityId {
        self.issuer
    }
}

impl fmt::Debug for ScopedGrantRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ScopedGrantRef(..)")
    }
}
