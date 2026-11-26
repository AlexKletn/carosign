//! PAM Codes: Translation of platform outcome in response to library.
//!
//! ⚠️ The rule for which this file exists (document 07): **`AcceptedFactor` - not logging in.
//! `PAM_SUCCESS` It only comes back when the factor is accepted for that very request and the
//! policy allows it. Any refusal that doesn't prove "wrong user" gives you the right answer.
//! `PAM_AUTHINFO_UNAVAIL`So that the backup path (password) remains possible.
//!
//! ⚠️ A false proof never succeeds, even if everything else has “passed.”

mod outcome;
mod pam_code;

pub use outcome::Outcome;
pub use pam_code::PamCode;
pub use pam_code::to_code;
