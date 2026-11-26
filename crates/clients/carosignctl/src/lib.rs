//! Management shell: analysis of commands, output and service client.
//!
//! ⚠️ The shell is the client, not the owner: it does not decide on authority and does not
//! receive any templates or evidence (document 07).

pub mod client;
pub mod commands;
pub mod output;
