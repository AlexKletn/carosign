mod enrollment_record;
mod enrollment_ref;
/// Communications `carosign.storage.v1`generated from the platform scheme.
///
/// ⚠️ Just the ones the registry needs, pull the whole thing. `storage.proto` In storage,
/// it's dependent on a circuit that we don't understand.
mod stored_value_record;
mod stored_value_kind;
mod subject_ref;

pub use enrollment_record::EnrollmentRecord;
pub use enrollment_ref::EnrollmentRef;
pub use stored_value_record::StoredValue;
pub use subject_ref::SubjectRef;

pub mod stored_value {
    pub use super::stored_value_kind::Value;
}
