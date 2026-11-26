//! Powers: permission limits, recheck points, revocation.
//!
//! ⚠️ Here ** form only**: what the permit outlines, not why it is
//! issued.`carosign-runtime::authority`The current open permission constructor does not, by
//! itself, establish trusted origin.
//!
//! Attention to two things:
//!
//! * [`ScopedGrantRef`**link, not right: it can be transmitted, but
//! The order is issued only by the [[Official]].`ValidatedGrant`].
//! * [`ValidatedGrant::issue_for_port`] is open and therefore does not prove
//! It's a trusted port, and it's an unfinished line of trust.

mod authorization_boundary;
mod grant_scope;
mod scoped_grant_ref;
mod validated_grant;

pub use authorization_boundary::AuthorizationBoundary;
pub use grant_scope::GrantScope;
pub use scoped_grant_ref::ScopedGrantRef;
pub use validated_grant::ValidatedGrant;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
