/// Observer limit on operation (Document 13).
pub const OBSERVERS_MAX: usize = 8;

/// The upper limit of the outgoing queue of one observer.
pub const OBSERVER_QUEUE_MAX: usize = super::super::events::HISTORY_MAX;

pub const OBSERVERS_HARD_MAX: usize = 32;
