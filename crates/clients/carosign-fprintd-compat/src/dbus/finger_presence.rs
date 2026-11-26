use dbus::{
    Message,
    arg::{PropMap, Variant},
    blocking::Connection,
    channel::Sender,
};
use std::error::Error;

use super::DEVICE;

#[derive(Default)]
pub(super) struct FingerPresence {
    pub(super) present: bool,
    pub(super) needed: bool,
}

impl FingerPresence {
    pub(super) fn update(
        &mut self,
        connection: &Connection,
        path: &str,
        present: Option<bool>,
        needed: Option<bool>,
    ) -> Result<(), Box<dyn Error>> {
        let mut changed = PropMap::new();
        if let Some(present) = present
            && present != self.present
        {
            self.present = present;
            changed.insert("finger-present".into(), Variant(Box::new(present)));
        }
        if let Some(needed) = needed
            && needed != self.needed
        {
            self.needed = needed;
            changed.insert("finger-needed".into(), Variant(Box::new(needed)));
        }
        if !changed.is_empty() {
            let signal =
                Message::new_signal(path, "org.freedesktop.DBus.Properties", "PropertiesChanged")?
                    .append3(DEVICE, changed, Vec::<String>::new());
            connection.send(signal).map_err(|_| "D-Bus signal failed")?;
        }
        Ok(())
    }

    pub(super) fn reset(
        &mut self,
        connection: &Connection,
        path: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.update(connection, path, Some(false), Some(false))
    }
}
