//! Device Profile: On what grounds the provider is suitable.
//!
//! ⚠️ Rule 09: Matching is done **only by inventory **, not by running the ISP's code, so
//! there's no single path to hardware here: just rules of comparison with what the platform
//! already sees.
//!
//! ⚠️ The matching of the profile gives neither rights nor guarantees: it only allows you to
//! consider the candidate.

mod match_rule;
mod matching;
mod profile;
mod profile_error;

pub use match_rule::MatchRule;
pub use matching::matches;
pub use profile::Profile;
pub use profile::parse;
pub use profile_error::ProfileError;
