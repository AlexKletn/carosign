use super::{BoxFuture, CleanupScope, ResourceDenial};

/// A live issue checker for a changing command, chosen by a trusted command; the matching set of
/// fields is not an authority.
pub(crate) trait MutationLeaseVerifier: Send + Sync {
    fn validate<'a>(&'a self, scope: &'a CleanupScope)
    -> BoxFuture<'a, Result<(), ResourceDenial>>;
}
