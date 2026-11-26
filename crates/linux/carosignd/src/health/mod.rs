//! Status shot from the owner of the service, no authority on the device.

#[path = "health.tests.rs"]
#[cfg(test)]
mod tests;

mod health_snapshot;
pub use health_snapshot::HealthSnapshot;
mod health_unavailable;
pub use health_unavailable::HealthUnavailable;
mod health_source;
pub use health_source::HealthSource;
mod unavailable_health;
pub use unavailable_health::UnavailableHealth;
