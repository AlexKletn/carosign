//! Talking to PAM: What to say to a person and what to ask.
//!
//! ⚠️ Texts - ** from an event listing**, not from the provider (document 07): the free text
//! of the provider is considered diagnostic, not an indication.

mod prompt;
mod response;

pub use prompt::Prompt;
pub use response::Response;
