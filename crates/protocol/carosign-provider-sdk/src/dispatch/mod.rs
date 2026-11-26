//! Distribution of commands: one command - one way, exactly one result.
//!
//! ⚠️ Rule 06: For the command, the worker gives exactly one** normal result, repeating the
//! result is a session failure, not "Okay, two answers."
//!
//! ⚠️ `CommandAccepted` It does not mean that hardware performs: it is only a reception.

mod action;
mod duplicate_terminal;
mod refusal;
mod terminal_ledger;

pub use action::Action;
pub use action::admittance;
pub use duplicate_terminal::DuplicateTerminal;
pub use refusal::refusal;
pub use terminal_ledger::TerminalLedger;
