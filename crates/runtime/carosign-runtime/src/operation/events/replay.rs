use super::{Event, OperationSnapshot};

/// The answer to repeat: either the surviving events, or the skip with the picture.
///
/// The image is created by the owner of the state after the observer is authorized, and for a
/// terminal change, its outcome must come from the repository.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Replay {
    /// The entire story requested has been preserved.
    Events(Vec<Event>),
    /// Pass and state shot on the last flow number.
    Gap {
        /// The last number known to the observer.
        after_sequence: u64,
        /// Number of the last event at the time of the photo creation.
        last_sequence: u64,
        /// Photo from a trusted source.
        snapshot: OperationSnapshot,
    },
}
