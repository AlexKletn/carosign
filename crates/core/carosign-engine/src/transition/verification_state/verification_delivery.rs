/// The stage of transmitting a positive result in the current launch.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VerificationDelivery {
    /// There's no confirmed record yet.
    #[default]
    Pending,
    /// Full response recorded; receipt by the customer is not confirmed.
    Sent,
}
