use super::EnrollmentRecord;

/// The result of the registry reading.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Lookup {
    /// We got it.
    Found(EnrollmentRecord),
    /// No record.
    Missing,
}
