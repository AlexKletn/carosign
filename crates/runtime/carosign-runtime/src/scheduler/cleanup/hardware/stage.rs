/// Consecutive stages of verification of one issue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Bound,
    AdmissionsClosed,
    InterruptsDrained,
    CallsDrained,
    DeviceQuiesced,
}
