use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::common::v1::InteractionMode;
use dbus::blocking::Connection;

use crate::system_backend::{SOCKET, fingerprints};

use super::{device::Device, device_state::DeviceState};

#[derive(Default)]
pub(super) struct DeviceDirectory<'a> {
    pub(super) devices: BTreeMap<usize, Device<'a>>,
    by_id: BTreeMap<Vec<u8>, usize>,
    next_path: usize,
    checked: Option<Instant>,
    last_success: Option<Instant>,
}

impl<'a> DeviceDirectory<'a> {
    pub(super) fn refresh(
        &mut self,
        connection: &'a Connection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        if self
            .checked
            .is_some_and(|checked| now.duration_since(checked) < Duration::from_millis(500))
        {
            return Ok(());
        }
        self.checked = Some(now);
        let inventory = Transport::connect_root(SOCKET)
            .ok()
            .and_then(|transport| fingerprints(&transport));
        let was_visible = !self.devices.is_empty();
        crate::diagnostics::event(
            if inventory
                .as_ref()
                .is_some_and(|devices| !devices.is_empty())
            {
                "availability_observed_present"
            } else {
                "availability_observed_absent"
            },
        );
        let inventory = match inventory {
            Some(devices) => {
                self.last_success = Some(now);
                devices
            }
            None if self
                .last_success
                .is_some_and(|success| now.duration_since(success) < Duration::from_secs(2)) =>
            {
                crate::diagnostics::event("availability_grace_retained");
                return Ok(());
            }
            None => Vec::new(),
        };
        let mut published = BTreeSet::new();
        for descriptor in inventory {
            let id = descriptor.authenticator_id;
            let index = if let Some(index) = self.by_id.get(&id) {
                *index
            } else {
                // A removed object's path is not reused for a different authenticator.
                let index = self.next_path;
                self.next_path = self
                    .next_path
                    .checked_add(1)
                    .ok_or("Device path overflow")?;
                self.by_id.insert(id.clone(), index);
                index
            };
            published.insert(index);
            if self
                .devices
                .get(&index)
                .is_some_and(|device| device.epoch != Some(descriptor.device_epoch))
            {
                self.remove(connection, index)?;
            }
            self.devices.entry(index).or_insert_with(|| {
                let swipe = descriptor.capabilities.as_ref().is_some_and(|value| {
                    value
                        .interaction_modes
                        .contains(&(InteractionMode::InteractionSwipe as i32))
                });
                let stages = descriptor
                    .capabilities
                    .as_ref()
                    .and_then(|value| value.enrollment_stages);
                let name = descriptor
                    .capabilities
                    .map(|value| value.display_name)
                    .filter(|name| {
                        !name.trim().is_empty()
                            && name.len() <= 128
                            && !name.chars().any(char::is_control)
                    });
                Device::system(connection, id, descriptor.device_epoch, swipe, name, stages)
            });
        }
        let removed: Vec<_> = self
            .devices
            .keys()
            .copied()
            .filter(|index| !published.contains(index))
            .collect();
        for index in removed {
            self.remove(connection, index)?;
        }
        match (was_visible, !self.devices.is_empty()) {
            (false, true) => crate::diagnostics::event("availability_became_present"),
            (true, false) => crate::diagnostics::event("availability_became_absent"),
            _ => {}
        }
        Ok(())
    }

    fn remove(
        &mut self,
        connection: &Connection,
        index: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut device) = self.devices.remove(&index) {
            device.state.unpublish(
                connection,
                device.backend.backend(),
                &DeviceState::path(index),
            )?;
        }
        Ok(())
    }
}
