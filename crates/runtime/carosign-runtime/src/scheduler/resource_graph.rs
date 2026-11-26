//! Resources Graph: Who's holding who?
//!
//! ⚠️ In this version, the graph matches the scheduler’s table: a separate graph will be
//! needed when there are general listeners (QSEE/RPMB), where “compatible” is defined not
//! only by the mode, but also by confirmed hardware security (document 09).

pub use super::{GrantResult, Scheduler};
