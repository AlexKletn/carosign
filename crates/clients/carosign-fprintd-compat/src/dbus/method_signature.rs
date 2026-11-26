pub(super) const MANAGER_PATH: &str = "/net/reactivated/Fprint/Manager";
pub(super) const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
pub(super) const INTROSPECTABLE: &str = "org.freedesktop.DBus.Introspectable";

pub(super) fn signature(path: &str, interface: &str, member: &str) -> Option<&'static str> {
    use super::{DEVICE, DEVICE_PATH, MANAGER};
    match (path, interface, member) {
        (MANAGER_PATH, MANAGER, "GetDevices" | "GetDefaultDevice") => Some(""),
        (MANAGER_PATH | DEVICE_PATH, INTROSPECTABLE, "Introspect") => Some(""),
        (MANAGER_PATH | DEVICE_PATH, PROPERTIES, "Get") => Some("ss"),
        (MANAGER_PATH | DEVICE_PATH, PROPERTIES, "GetAll") => Some("s"),
        (MANAGER_PATH | DEVICE_PATH, PROPERTIES, "Set") => Some("ssv"),
        (
            DEVICE_PATH,
            DEVICE,
            "Claim"
            | "ListEnrolledFingers"
            | "DeleteEnrolledFingers"
            | "DeleteEnrolledFinger"
            | "VerifyStart"
            | "EnrollStart",
        ) => Some("s"),
        (
            DEVICE_PATH,
            DEVICE,
            "Release" | "DeleteEnrolledFingers2" | "VerifyStop" | "EnrollStop",
        ) => Some(""),
        _ => None,
    }
}

pub(super) fn matches_input(message: &dbus::Message, expected: &str) -> bool {
    let mut arguments = message.iter_init();
    for kind in expected.bytes() {
        if arguments.arg_type() as u8 != kind {
            return false;
        }
        arguments.next();
    }
    arguments.arg_type() == dbus::arg::ArgType::Invalid
}
