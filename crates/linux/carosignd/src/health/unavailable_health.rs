use super::{HealthSnapshot, HealthSource, HealthUnavailable};

/// Closed version for the composition without the source of the state.
pub struct UnavailableHealth;

impl HealthSource for UnavailableHealth {
    fn snapshot(&self) -> Result<HealthSnapshot, HealthUnavailable> {
        Err(HealthUnavailable)
    }
}
