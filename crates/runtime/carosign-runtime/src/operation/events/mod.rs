//! Operation Event Flow: Sequence, Sticking, and Dips.
//!
//! ⚠️ The event number grows ** by one operation ** and starts with 1.
//! The final result of the change must come from a stable repository; the receipt of an
//! authorized result is not yet connected here.
//!
//! ⚠️ The progress can be glued together. The result of the change cannot be (Document 05,
//! rule 6).

mod capture_percent;
mod event;
mod event_error;
mod event_kind;
mod event_stream;
mod limits;
mod operation_snapshot;
mod progress;
mod replay;
mod snapshot_error;

pub use capture_percent::CapturePercent;
pub use event::Event;
pub use event_error::EventError;
pub use event_kind::EventKind;
pub use event_stream::EventStream;
pub use limits::HISTORY_MAX;
pub use operation_snapshot::OperationSnapshot;
pub use progress::Progress;
pub use replay::Replay;
pub use snapshot_error::SnapshotError;
