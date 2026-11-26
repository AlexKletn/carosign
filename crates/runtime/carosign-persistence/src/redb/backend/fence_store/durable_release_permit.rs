use super::FenceScope;

/// One-time authority to remove the fencing of a specific issue.
/// Coincidence of identifiers does not in itself create such a power.
pub struct DurableReleasePermit(pub(super) FenceScope);

impl DurableReleasePermit {
    pub fn scope(&self) -> &FenceScope {
        &self.0
    }

    /// Creates an authority after verifying a trusted cleaning confirmation.
    ///
    /// # Safety
    /// The caller is obliged to confirm the physical quiescence of all participants of this issue
    /// and exclude the resumption of access until the fencing is removed.
    /// An error here may result in the re-issuing of a memory or device to which the former
    /// owner is still accessing.
    pub unsafe fn from_verified_quiescence(scope: FenceScope) -> Self {
        Self(scope)
    }
}
