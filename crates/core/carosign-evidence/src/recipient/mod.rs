//! Recipient and call: to whom and what question is answered.
//!
//! ⚠️ The rule that is most often violated is that **challenge is created by a secure
//! recipient or trusted source of authority, not an unprivileged client. A random number from
//! a client does not become evidence (document 11).

mod challenge_ref;
mod recipient_ref;

pub use challenge_ref::ChallengeRef;
pub use recipient_ref::RecipientRef;
