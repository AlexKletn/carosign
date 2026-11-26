/// Observer notifications are processed without waiting for a socket in the loop.
pub type EventNotification<'a> =
    dyn FnMut(Result<Vec<u32>, carosign_runtime::operation::events::EventError>) + 'a;
