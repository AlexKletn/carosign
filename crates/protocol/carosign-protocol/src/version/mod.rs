//! Wire version and minor matching.
//!
//! ⚠️ The older version must match: the other older one corresponds to a different meaning of
//! the fields. Minor is chosen as **the greatest common **; unfamiliar features are not
//! considered included (document 06).
//!
//! ⚠️ The corridor of the younger versions is **supported** (not "basic"): the minor of the
//! remote side must cross with the supported ones.

mod negotiated;
mod version_mismatch;

pub use negotiated::Negotiated;
pub use negotiated::negotiate;
pub use version_mismatch::VersionMismatch;
