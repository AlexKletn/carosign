use super::ProjectionChange;

/// The projection is complete.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Projection {
    /// Verified change.
    pub change: ProjectionChange,
    /// How many records were in the area picture.
    pub scope_size: usize,
}
