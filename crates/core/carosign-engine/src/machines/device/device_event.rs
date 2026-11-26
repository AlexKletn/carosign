/// What happened to the device?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceEvent {
    /// Resources taken.
    Reserved,
    /// Work has begun.
    Started,
    /// Please calm down.
    Quiesce,
    /// Reassurance confirmed.
    Quiesced,
    /// Failure: Status unknown.
    Fault,
    /// Recovery has begun.
    Recover,
    /// We're out of touch.
    Disconnected,
    /// Reconnection is confirmed by the trusted owner of the device.
    Reconnected,
}
