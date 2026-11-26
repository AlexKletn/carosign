use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::carosign::provider::v1 as provider;

/// The difference between creation and replacement preserves the entire plan transmitted by
/// the platform.
#[derive(Clone, PartialEq)]
pub enum EnrollmentChange {
    Create {
        planned: common::EnrollmentRef,
    },
    Replace {
        planned: common::EnrollmentRef,
        old_binding: provider::EnrollmentBinding,
    },
}
