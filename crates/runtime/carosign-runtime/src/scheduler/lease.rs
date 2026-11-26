//! Leasing issues: what to whom and in what era.
//!
//! ⚠️ The issuance belongs to the era. The change of an era does not free a resource by
//! itself, but it must be noticed by the one who issued it and called upon to do so.
//! `release`.

pub use super::Holder;
