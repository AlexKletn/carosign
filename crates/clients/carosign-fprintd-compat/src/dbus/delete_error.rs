#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteError {
    NoEnrolledPrints,
    PrintsNotDeleted,
    PrintsNotDeletedFromDevice,
}

impl DeleteError {
    pub(super) fn reply(self, message: &dbus::Message) -> dbus::Message {
        let (name, explanation) = match self {
            Self::NoEnrolledPrints => ("NoEnrolledPrints", c"No matching enrollment"),
            Self::PrintsNotDeleted => ("PrintsNotDeleted", c"Deletion was not confirmed"),
            Self::PrintsNotDeletedFromDevice => (
                "PrintsNotDeletedFromDevice",
                c"Device deletion was not confirmed",
            ),
        };
        super::method_error::error(
            message,
            &format!("net.reactivated.Fprint.Error.{name}"),
            explanation,
        )
    }
}
