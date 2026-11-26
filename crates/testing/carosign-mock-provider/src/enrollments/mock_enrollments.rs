use carosign_protocol::carosign::common::v1 as common;

pub(crate) type MockEnrollments =
    std::collections::BTreeMap<Vec<u8>, (common::SubjectRef, common::EnrollmentRef, Vec<u8>)>;
