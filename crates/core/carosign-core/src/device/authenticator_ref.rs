use super::DeviceEpoch;
use crate::ids::AuthenticatorId;
use core::fmt;

/// Reference to the authenticator along with the era of binding.
///
/// The era is imperative: without it, an outdated worker message from a previous launch
/// would have passed as current.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthenticatorRef {
    /// Stable device identifier.
    pub id: AuthenticatorId,
    /// The age of anchorage.
    pub device_epoch: DeviceEpoch,
}

impl fmt::Debug for AuthenticatorRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AuthenticatorRef(.., epoch={})",
            self.device_epoch.value()
        )
    }
}
