use super::{Priority, Request};
use carosign_core::operation::context::BootInstant;

/// Default queue limit (document 13:16 per device).
///
/// Increase to the ceiling requires a separate configuration; since there is no such
/// configuration, it is the base limit that is used, not the absolute ceiling.
pub const QUEUE_MAX: usize = 16;

/// Who got the slot after waiting.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StarvationTick {
    /// How long the waiter waited, ns.
    pub waited_ns: u64,
    /// Did you get the slot?
    pub granted: bool,
}

/// Anti-fasting threshold (document 22): 30 seconds.
pub const STARVATION_NS: u64 = 30_000_000_000;

/// Did the interactive request reach the threshold within the same download?
pub fn is_starving(now: BootInstant, request: &Request) -> bool {
    request.priority == Priority::InteractiveManage
        && now.boot() == request.since.boot()
        && now.instant_ns().saturating_sub(request.since.instant_ns()) > STARVATION_NS
}
