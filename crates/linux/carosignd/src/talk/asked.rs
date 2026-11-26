use super::{DeviceEpoch, SubjectRef};

/// What the client asks to check: the subject, the apparatus, the era.
///
/// The client identifies the current connection user. The identity source resolves the
/// subject. The frame carries the authenticator ID, which grants no permission. See
/// specification 07.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Asked {
    /// We're checking.
    pub subject: SubjectRef,
    /// On what device.
    pub epoch: DeviceEpoch,
}
