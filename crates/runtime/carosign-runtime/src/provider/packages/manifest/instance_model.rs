/// How many workers per device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceModel {
    /// One on the device.
    PerDevice,
}
