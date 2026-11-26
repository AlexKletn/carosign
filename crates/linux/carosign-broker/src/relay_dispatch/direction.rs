/// The kind of permitted direction; it is not in itself a policy resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Normal command, no descriptors.
    Command,
    /// Resource binding, the only place to transmit descriptors.
    Bind,
    /// An worker event.
    Event,
    /// A cancellation that is formed only by a mediator.
    BrokerCancel,
    EnrollmentTransaction,
}
