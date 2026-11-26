//! Actor mail: two lines, management priority.
//!
//! ⚠️ Rule 22: ** governance first, then the usual** Withdrawal of authority must not wait
//! until the normal turn of execution is over, otherwise success will be delivered after the
//! withdrawal.
//!
//! ⚠️ The queues are **limited. Control overflows are not "lost the message," but a reason to
//! close the operation with a safe refusal: you can't lose feedback.

mod limits;
mod mailbox;
mod mailbox_full;
mod priority;

pub use limits::CONTROL_BURST;
pub use limits::CONTROL_CAPACITY;
pub use limits::NORMAL_CAPACITY;
pub use mailbox::Mailbox;
pub use mailbox_full::MailboxFull;
pub use priority::Priority;
