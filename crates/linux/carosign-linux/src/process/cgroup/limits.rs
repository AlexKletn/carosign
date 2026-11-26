use std::num::NonZeroU64;

/// The numerical limits for one worker approved by the composition of the service.
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) cpu_quota_us: NonZeroU64,
    pub(crate) cpu_period_us: NonZeroU64,
    pub(crate) memory_bytes: NonZeroU64,
    pub(crate) processes: NonZeroU64,
}
