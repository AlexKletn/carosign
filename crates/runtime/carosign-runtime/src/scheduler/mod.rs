//! Resource scheduler: queue by class and atomic delivery of the set.
//!
//! ⚠️ The policy is enshrined in document 22 and only the ADR changes: `SystemAuth >
//! InteractiveManage > Diagnostic`, FIFO within the class, an interactive request that waits
//! longer than 30 seconds must receive the next free slot, but **never** will not supersede
//! the ongoing change. `dispatch_at` or `release_at`; the previous challenges without time
//! increase do not fulfill.
//!
//! ⚠️ The kit is taken completely and atomically: partial issuance is impossible, it has no
//! one owner and no one to release it.

pub mod cleanup;

mod holder;
mod priority;
mod queue;
mod quiescing;
mod request;
mod scheduler;

pub use holder::Holder;
pub use priority::Priority;
pub use queue::{QUEUE_MAX, STARVATION_NS, StarvationTick, is_starving};
pub use quiescing::{FenceStage, Quiescing};
pub use request::{ExpiredRequest, GrantResult, Request};
pub use scheduler::{RestoreFenceError, Scheduler};
