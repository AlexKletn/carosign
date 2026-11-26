//! Provider Requirements for Resources
//!
//! ⚠️ The provider declares the need, and gives it to the broker. `ResourceSet` It's
//! just a description, it doesn't give anything.
//!
//! Order canonical: the set is sorted by by bytes `ResourceId` lexicographically (document
//! 09: “Scheduler sorts all `ResourceId` Bytes lexicographically: Without a single order, two
//! transactions with the same resources, taken in different order, would force the
//! broker to issue incomplete leasings.

mod access_mode;
mod invalid_resource_set;
mod resource_claim;
mod resource_set;

pub use access_mode::AccessMode;
pub use invalid_resource_set::InvalidResourceSet;
pub use resource_claim::ResourceClaim;
pub use resource_set::{RESOURCE_SET_MAX, ResourceSet};

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
