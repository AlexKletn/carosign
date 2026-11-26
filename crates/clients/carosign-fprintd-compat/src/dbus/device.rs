use super::{device_backend::DeviceBackend, device_state::DeviceState};
use crate::system_backend::SystemBackend;

pub(super) struct Device<'a> {
    pub(super) state: DeviceState,
    pub(super) epoch: Option<u64>,
    pub(super) backend: DeviceBackend<'a>,
}

impl<'a> Device<'a> {
    pub(super) fn new(backend: DeviceBackend<'a>) -> Self {
        Self {
            state: DeviceState::new(),
            epoch: None,
            backend,
        }
    }

    pub(super) fn system(
        connection: &'a dbus::blocking::Connection,
        id: Vec<u8>,
        epoch: u64,
        swipe: bool,
        name: Option<String>,
        stages: Option<u32>,
    ) -> Self {
        let mut backend = SystemBackend::new(connection);
        backend.select_device(id, swipe, name);
        let mut device = Self::new(DeviceBackend::System(backend));
        device.epoch = Some(epoch);
        device.state = device.state.with_declared_stages(stages);
        device
    }
}
