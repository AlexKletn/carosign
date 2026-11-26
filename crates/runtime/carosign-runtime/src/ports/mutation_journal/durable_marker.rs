/// Confirmation of stable record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DurableMarker {
    /// It's on the disc and it's confirmed.
    pub durable: bool,
}
