#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartError {
    PermissionDenied,
    AlreadyInUse,
    NoEnrolledPrints,
    FingerAlreadyEnrolled,
    Internal,
}

impl StartError {
    pub(super) fn reply(self, message: &dbus::Message) -> dbus::Message {
        let (name, description) = match self {
            Self::PermissionDenied => ("PermissionDenied", c"Permission denied"),
            Self::AlreadyInUse => ("AlreadyInUse", c"Device is busy"),
            Self::NoEnrolledPrints => ("NoEnrolledPrints", c"No eligible enrolled fingerprints"),
            Self::FingerAlreadyEnrolled => ("FingerAlreadyEnrolled", c"Finger is already enrolled"),
            Self::Internal => ("Internal", c"Could not prepare the operation"),
        };
        super::method_error::error(
            message,
            &format!("net.reactivated.Fprint.Error.{name}"),
            description,
        )
    }
}
