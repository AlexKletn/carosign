use dbus::blocking::Connection;
use std::{collections::HashMap, time::Duration};

pub(super) fn authorize(bus: &Connection, caller: &str, uid: u32, operation: &str) -> bool {
    let proxy = bus.with_proxy(
        "org.freedesktop.PolicyKit1",
        "/org/freedesktop/PolicyKit1/Authority",
        Duration::from_secs(120),
    );
    let mut subject = dbus::arg::PropMap::new();
    subject.insert(
        "name".into(),
        dbus::arg::Variant(Box::new(caller.to_owned())),
    );
    let action = format!("net.reactivated.fprint.device.{operation}");
    let result: Result<((bool, bool, HashMap<String, String>),), _> = proxy.method_call(
        "org.freedesktop.PolicyKit1.Authority",
        "CheckAuthorization",
        (
            ("system-bus-name", subject),
            action,
            HashMap::<String, String>::new(),
            1u32,
            "",
        ),
    );
    match result {
        Ok(((authorized, _, _),)) => authorized,
        Err(error) => {
            uid == 0
                && matches!(
                    error.name(),
                    Some(
                        "org.freedesktop.DBus.Error.ServiceUnknown"
                            | "org.freedesktop.DBus.Error.NameHasNoOwner"
                    )
                )
        }
    }
}

pub(super) fn authorize_async(
    caller: &str,
    uid: u32,
    target_uid: u32,
    actions: &'static [&'static str],
) -> Option<std::sync::mpsc::Receiver<Option<u32>>> {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };

    static ACTIVE: AtomicUsize = AtomicUsize::new(0);
    ACTIVE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
            (count < 16).then_some(count + 1)
        })
        .ok()?;
    let caller = caller.to_owned();
    let (send, receive) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("fprintd-policy".into())
        .spawn(move || {
            let allowed = Connection::new_system().is_ok_and(|bus| {
                (uid == target_uid || authorize(&bus, &caller, uid, "setusername"))
                    && actions
                        .iter()
                        .any(|action| authorize(&bus, &caller, uid, action))
            });
            ACTIVE.fetch_sub(1, Ordering::Relaxed);
            let _ = send.send(allowed.then_some(target_uid));
        });
    if worker.is_err() {
        ACTIVE.fetch_sub(1, Ordering::Relaxed);
        return None;
    }
    Some(receive)
}
