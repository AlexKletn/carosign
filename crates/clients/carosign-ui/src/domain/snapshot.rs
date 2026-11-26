use carosign_protocol::carosign::client::v1::{Authenticators, Health};

use carosign_protocol::carosign::common::v1::EnrollmentSummary;

use super::QueryError;

#[derive(Clone, PartialEq)]
pub(crate) struct Snapshot {
    pub(crate) health: Result<Health, QueryError>,
    pub(crate) enrollments: Result<Vec<EnrollmentSummary>, QueryError>,
    pub(crate) inventory: Result<Authenticators, QueryError>,
}
