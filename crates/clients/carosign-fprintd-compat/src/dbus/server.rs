use super::{
    Backend, MANAGER, MANAGER_XML, UnavailableBackend,
    device::Device,
    device_backend::DeviceBackend,
    device_directory::DeviceDirectory,
    device_state::DeviceState,
    method_error::{error, method_error},
    method_signature::{self, INTROSPECTABLE, MANAGER_PATH, PROPERTIES},
};
use dbus::{Message, Path, blocking::Connection, channel::Sender, message::MessageType};
use std::{error::Error, time::Duration};

pub fn serve_system() -> Result<(), Box<dyn Error>> {
    let connection = Connection::new_system()?;
    serve_directory(&connection, DeviceDirectory::default(), true, || false)
}

pub fn serve(connection: &Connection) -> Result<(), Box<dyn Error>> {
    serve_with_backend(connection, &mut UnavailableBackend, || false)
}

pub fn serve_with_backend(
    connection: &Connection,
    backend: &mut dyn Backend,
    should_stop: impl FnMut() -> bool,
) -> Result<(), Box<dyn Error>> {
    let mut directory = DeviceDirectory::default();
    directory
        .devices
        .insert(0, Device::new(DeviceBackend::Supplied(backend)));
    serve_directory(connection, directory, false, should_stop)
}

fn serve_directory<'a>(
    connection: &'a Connection,
    mut directory: DeviceDirectory<'a>,
    discover: bool,
    mut should_stop: impl FnMut() -> bool,
) -> Result<(), Box<dyn Error>> {
    use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
    if connection.request_name("net.reactivated.Fprint", false, false, true)?
        != RequestNameReply::PrimaryOwner
    {
        return Err("The fprintd bus name is already owned".into());
    }
    connection.add_match_no_cb("type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged'")?;
    let result = (|| {
        while !should_stop() {
            if discover {
                directory.refresh(connection)?;
            }
            for (&index, device) in &mut directory.devices {
                device.state.poll(
                    connection,
                    device.backend.backend(),
                    &DeviceState::path(index),
                )?;
            }
            let Some(message) = connection
                .channel()
                .blocking_pop_message(Duration::from_millis(50))?
            else {
                continue;
            };
            if let Some(name) = disconnected(&message) {
                for (&index, device) in &mut directory.devices {
                    device.state.disconnect(
                        connection,
                        device.backend.backend(),
                        &DeviceState::path(index),
                        name,
                    )?;
                }
            }
            if message.msg_type() == MessageType::MethodCall {
                dispatch(connection, &mut directory, message)?;
            }
        }
        Ok(())
    })();
    for device in directory.devices.values_mut() {
        device.state.stop(device.backend.backend());
    }
    result
}

fn disconnected(message: &Message) -> Option<&str> {
    if message.msg_type() != MessageType::Signal
        || !message
            .sender()
            .is_some_and(|value| value == "org.freedesktop.DBus")
        || !message
            .interface()
            .is_some_and(|value| value == "org.freedesktop.DBus")
        || !message
            .member()
            .is_some_and(|value| value == "NameOwnerChanged")
    {
        return None;
    }
    let (name, previous, current) = message.read3::<&str, &str, &str>().ok()?;
    (name == previous && current.is_empty()).then_some(name)
}

fn dispatch(
    connection: &Connection,
    directory: &mut DeviceDirectory<'_>,
    message: Message,
) -> Result<(), Box<dyn Error>> {
    let interface = message
        .interface()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let member = message
        .member()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let path = message
        .path()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let caller = message
        .sender()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let reply = if path == MANAGER_PATH {
        if let Some(invalid) = invalid_args(&message, &path, &interface, &member) {
            invalid
        } else {
            manager_reply(directory, &message, &interface, &member)?
        }
    } else if let Some(index) = DeviceState::index(&path)
        && let Some(device) = directory.devices.get_mut(&index)
        && (device.state.claimed() || device.backend.backend().available())
    {
        if let Some(invalid) = invalid_args(&message, super::DEVICE_PATH, &interface, &member) {
            invalid
        } else {
            return device.state.dispatch_with_authorization(
                connection,
                device.backend.backend(),
                &path,
                &interface,
                &member,
                &caller,
                message,
            );
        }
    } else {
        error(
            &message,
            "org.freedesktop.DBus.Error.UnknownObject",
            c"Unknown object",
        )
    };
    connection.send(reply).map_err(|_| "D-Bus reply failed")?;
    Ok(())
}

fn invalid_args(message: &Message, path: &str, interface: &str, member: &str) -> Option<Message> {
    method_signature::signature(path, interface, member)
        .filter(|signature| !method_signature::matches_input(message, signature))
        .map(|_| {
            error(
                message,
                "org.freedesktop.DBus.Error.InvalidArgs",
                c"Invalid arguments",
            )
        })
}

fn manager_reply(
    directory: &mut DeviceDirectory<'_>,
    message: &Message,
    interface: &str,
    member: &str,
) -> Result<Message, Box<dyn Error>> {
    Ok(match (interface, member) {
        (INTROSPECTABLE, "Introspect") => message.return_with_args((MANAGER_XML,)),
        (MANAGER, "GetDevices" | "GetDefaultDevice") => {
            if member == "GetDefaultDevice" {
                crate::diagnostics::event("get_default_device_request");
            }
            let devices: Vec<_> = directory
                .devices
                .iter_mut()
                .filter_map(|(&index, device)| {
                    device
                        .backend
                        .backend()
                        .available()
                        .then(|| Path::new(DeviceState::path(index)))
                })
                .collect::<Result<_, _>>()?;
            if member == "GetDevices" {
                message.return_with_args((devices,))
            } else if let Some(device) = devices.first() {
                crate::diagnostics::event("get_default_device_reply");
                message.return_with_args((device,))
            } else {
                crate::diagnostics::event("get_default_device_error");
                error(
                    message,
                    "net.reactivated.Fprint.Error.NoSuchDevice",
                    c"No device",
                )
            }
        }
        (PROPERTIES, "GetAll") if message.read1::<&str>().ok() == Some(MANAGER) => {
            message.return_with_args((dbus::arg::PropMap::new(),))
        }
        _ => method_error(message, MANAGER_PATH, interface, member, false, false, None),
    })
}
