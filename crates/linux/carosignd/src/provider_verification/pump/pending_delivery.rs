use super::VerificationObservation;

pub(super) enum PendingDelivery {
    Accepted,
    AckQueued,
    Outcome(Box<VerificationObservation>),
}
