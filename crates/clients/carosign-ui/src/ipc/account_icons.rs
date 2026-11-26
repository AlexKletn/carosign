use std::{collections::HashMap, path::PathBuf, time::Duration};

use dbus::{
    Path,
    arg::RefArg,
    blocking::{Connection, stdintf::org_freedesktop_dbus::Properties},
};

pub(super) fn read() -> Result<HashMap<u32, PathBuf>, dbus::Error> {
    const SERVICE: &str = "org.freedesktop.Accounts";
    const TIMEOUT: Duration = Duration::from_millis(300);
    let connection = Connection::new_system()?;
    let proxy = connection.with_proxy(SERVICE, "/org/freedesktop/Accounts", TIMEOUT);
    let (users,): (Vec<Path<'static>>,) = proxy.method_call(SERVICE, "ListCachedUsers", ())?;
    let icons = users
        .into_iter()
        .filter_map(|path| {
            let user = connection.with_proxy(SERVICE, path, TIMEOUT);
            let properties = user.get_all("org.freedesktop.Accounts.User").ok()?;
            let uid = u32::try_from(properties.get("Uid")?.0.as_u64()?).ok()?;
            let path = PathBuf::from(properties.get("IconFile")?.0.as_str()?);
            path.is_absolute().then_some((uid, path))
        })
        .collect();
    Ok(icons)
}
