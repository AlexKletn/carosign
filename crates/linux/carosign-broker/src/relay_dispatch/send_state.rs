/// The completion of send does not confirm the rest of the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendState {
    Prepared,
    Sending,
    Sent,
    Revoked,
    Uncertain,
}
