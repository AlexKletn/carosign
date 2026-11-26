//! Observes D-Bus senders without granting authority.

use dbus::arg::{OwnedFd, PropMap};
use dbus::blocking::Connection;
use std::os::fd::{AsRawFd, FromRawFd};
use std::time::Duration;

/// Non-transferable reference to the process behind a unique D-Bus name.
///
/// The caller must establish bus trust and operation binding and handle name loss.
pub struct ObservedSender {
    unique_name: String,
    process: OwnedFd,
    uid: u32,
    pid: u32,
}

impl ObservedSender {
    /// Returns the checked unique bus name.
    pub fn unique_name(&self) -> &str {
        &self.unique_name
    }

    /// Unverified credentials from the same bus reply as `ProcessFD`.
    pub fn credentials(&self) -> (u32, u32) {
        (self.uid, self.pid)
    }

    /// Checks whether the retained process is alive and fails closed on errors.
    pub fn is_alive(&self) -> bool {
        let mut p = libc::pollfd {
            fd: self.process.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll receives a valid pointer to one pollfd.
        unsafe { libc::poll(&mut p, 1, 0) == 0 }
    }

    /// Rechecks name ownership on the same bus and process liveness.
    pub fn still_owned(&self, bus: &Connection) -> bool {
        self.is_alive()
            && owner(bus, &self.unique_name).is_ok_and(|name| name == self.unique_name)
            && self.is_alive()
    }
}

fn owner(bus: &Connection, name: &str) -> Result<String, dbus::Error> {
    let proxy = bus.with_proxy(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        Duration::from_secs(2),
    );
    let (owner,): (String,) = proxy.method_call("org.freedesktop.DBus", "GetNameOwner", (name,))?;
    Ok(owner)
}

/// Observes a unique sender through ProcessFD on a bus trusted by the caller.
///
/// Checks the sender before and after obtaining the descriptor. The observation
/// confers no authority, bus trust, or right to delegate.
pub fn observe_sender(
    bus: &Connection,
    sender: &str,
) -> Result<ObservedSender, Box<dyn std::error::Error>> {
    if !sender.starts_with(':') || owner(bus, sender)? != sender {
        return Err("требуется уникальное имя действующего отправителя".into());
    }
    let proxy = bus.with_proxy(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        Duration::from_secs(2),
    );
    let (credentials,): (PropMap,) = proxy.method_call(
        "org.freedesktop.DBus",
        "GetConnectionCredentials",
        (sender,),
    )?;
    let credential = |name: &str| -> Result<u32, Box<dyn std::error::Error>> {
        let value = credentials
            .get(name)
            .ok_or("шина не предоставила учётные данные")?;
        if value.0.arg_type() != dbus::arg::ArgType::UInt32 {
            return Err("неожиданный тип учётных данных".into());
        }
        Ok(u32::try_from(
            value.0.as_u64().ok_or("недоступны учётные данные")?,
        )?)
    };
    let uid = credential("UnixUserID")?;
    let pid = credential("ProcessID")?;
    if pid == 0 {
        return Err("нулевой PID отправителя".into());
    }
    let value = credentials
        .get("ProcessFD")
        .ok_or("шина не предоставила ProcessFD")?;
    if value.0.arg_type() != dbus::arg::ArgType::UnixFd {
        return Err("неожиданный тип ProcessFD".into());
    }
    let original = i32::try_from(value.0.as_i64().ok_or("недоступен ProcessFD")?)?;
    // SAFETY: The descriptor being duplicated belongs to the live D-Bus reply.
    let duplicate = unsafe { libc::fcntl(original, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicate < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: A successful F_DUPFD_CLOEXEC returns a new owned descriptor.
    let fd = unsafe { OwnedFd::from_raw_fd(duplicate) };
    let observed = ObservedSender {
        unique_name: sender.to_owned(),
        process: fd,
        uid,
        pid,
    };
    if !observed.still_owned(bus) {
        return Err("отправитель исчез при наблюдении".into());
    }
    Ok(observed)
}
