use super::Backend;
use crate::system_backend::SystemBackend;

pub(super) enum DeviceBackend<'a> {
    System(SystemBackend<'a>),
    Supplied(&'a mut dyn Backend),
}

impl DeviceBackend<'_> {
    pub(super) fn backend(&mut self) -> &mut dyn Backend {
        match self {
            Self::System(backend) => backend,
            Self::Supplied(backend) => *backend,
        }
    }
}
