//! Cover commands: what can be asked from the line.
//!
//! ⚠️ The shell is a client: it doesn't circumvent authority, it doesn't get any templates,
//! it doesn't get any evidence, so it's just the actions listed in document 07.

mod command;
mod command_error;

pub use command::Command;
pub use command::parse;
pub use command_error::CommandError;
