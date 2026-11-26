use dbus::Message;

use super::{Action, DEVICE, DEVICE_PATH, MANAGER, method_signature::*};

pub(super) fn error(message: &Message, name: &str, description: &std::ffi::CStr) -> Message {
    message.error(&name.into(), description)
}

pub(super) fn method_error(
    message: &Message,
    path: &str,
    interface: &str,
    member: &str,
    claimed: bool,
    owns: bool,
    action: Option<Action>,
) -> Message {
    if !matches!(path, DEVICE_PATH | MANAGER_PATH) {
        return error(
            message,
            "org.freedesktop.DBus.Error.UnknownObject",
            c"Unknown object",
        );
    }
    let primary = if path == DEVICE_PATH { DEVICE } else { MANAGER };
    if !matches!(interface, PROPERTIES | INTROSPECTABLE) && interface != primary {
        return error(
            message,
            "org.freedesktop.DBus.Error.UnknownInterface",
            c"Unknown interface",
        );
    }
    if signature(path, interface, member).is_none() {
        return error(
            message,
            "org.freedesktop.DBus.Error.UnknownMethod",
            c"Unknown method",
        );
    }
    if interface == PROPERTIES {
        return property_error(message, primary, member);
    }
    if !claimed
        && matches!(
            member,
            "Release"
                | "VerifyStart"
                | "VerifyStop"
                | "EnrollStart"
                | "EnrollStop"
                | "DeleteEnrolledFingers2"
                | "DeleteEnrolledFinger"
        )
    {
        return error(
            message,
            "net.reactivated.Fprint.Error.ClaimDevice",
            c"Claim the device first",
        );
    }
    if (owns
        && action.is_some()
        && matches!(
            member,
            "VerifyStart" | "EnrollStart" | "DeleteEnrolledFingers2" | "DeleteEnrolledFinger"
        ))
        || (claimed && member == "DeleteEnrolledFingers")
        || (claimed && !owns && interface == DEVICE && member != "ListEnrolledFingers")
    {
        return error(
            message,
            "net.reactivated.Fprint.Error.AlreadyInUse",
            c"Device is busy",
        );
    }
    if owns && matches!(member, "VerifyStop" | "EnrollStop") {
        return error(
            message,
            "net.reactivated.Fprint.Error.NoActionInProgress",
            c"No action in progress",
        );
    }
    if owns && matches!(member, "DeleteEnrolledFingers2" | "DeleteEnrolledFinger") {
        return error(
            message,
            "net.reactivated.Fprint.Error.PrintsNotDeleted",
            c"Deletion did not start",
        );
    }
    error(
        message,
        "net.reactivated.Fprint.Error.PermissionDenied",
        c"Permission denied",
    )
}

fn property_error(message: &Message, interface: &str, member: &str) -> Message {
    if message.read1::<&str>().ok() != Some(interface) {
        return error(
            message,
            "org.freedesktop.DBus.Error.UnknownInterface",
            c"Unknown interface",
        );
    }
    if member == "Set"
        && interface == DEVICE
        && message.read2::<&str, &str>().is_ok_and(|(_, property)| {
            matches!(
                property,
                "name" | "scan-type" | "num-enroll-stages" | "finger-present" | "finger-needed"
            )
        })
    {
        return error(
            message,
            "org.freedesktop.DBus.Error.PropertyReadOnly",
            c"Property is read-only",
        );
    }
    error(
        message,
        "org.freedesktop.DBus.Error.UnknownProperty",
        c"Unknown property",
    )
}
