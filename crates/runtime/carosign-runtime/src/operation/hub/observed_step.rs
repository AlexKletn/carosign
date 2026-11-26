use crate::operation::events::EventError;

/// Actor's decision and independent publication result.
#[derive(Debug)]
pub struct ObservedStep {
    /// Solution and effects that cannot be lost when the observer fails.
    pub step: crate::operation::actor::Step,
    /// Connection identifiers to close or fail flow.
    pub notification: Result<Vec<u32>, EventError>,
}
