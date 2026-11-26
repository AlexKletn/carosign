use super::DeviceEpoch;
use crate::ids::AuthenticatorId;
use crate::ids::DeviceId;
use core::fmt;

/// Stable metadata of the device under the control of the platform.
///
/// A package name, a `/dev/*` path, and the chip model do not establish identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Authenticator {
    pub id: AuthenticatorId,
    /// The device to which the machine is attached.
    pub device: DeviceId,
    /// The age of anchorage.
    pub epoch: DeviceEpoch,
}

impl fmt::Debug for Authenticator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Authenticator(.., epoch={})", self.epoch.value())
    }
}
