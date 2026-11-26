//! Port assembly: the root of composition.
//!
//! ⚠️ Rule 02: Specific embodiments of ports are assembled in one place, here. No one below
//! knows who's behind the line.
//!
//! ⚠️ The assembly is obliged to refuse if the mandatory port is not specified. A service
//! that has risen with an empty source of authority is a service that lets everything go.

mod boot_error;
mod boot_report;
mod builder;

pub use boot_error::BootError;
pub use boot_report::BootReport;
pub use builder::Builder;
pub use builder::compose;
