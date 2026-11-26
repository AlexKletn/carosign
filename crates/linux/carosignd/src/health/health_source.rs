use super::{HealthSnapshot, HealthUnavailable};

/// The source returns the current agreed image or refusal.
pub trait HealthSource {
    /// Gets a picture at the time of the request.
    fn snapshot(&self) -> Result<HealthSnapshot, HealthUnavailable>;
}
