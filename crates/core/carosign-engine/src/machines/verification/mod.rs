//! Testing machine: from admission to the result.
//!
//! Table of transitions - document 04, section "Verification transition table".
//!
//! ⚠️ `ProviderMatched` **Never completes an operation with success: it leads to the
//! verification of the evidence, and only the accepted proof with the current owner gives the
//! proof.`TerminalOutcome::FactorAccepted`](crate::transition::TerminalOutcome::FactorAccepted).
//!
//! ⚠️ The owner’s recall and deadline are translated into cancellation from any non-terminal
//! phase; if the recall comes at the same time as the acceptance of the proof, suppression
//! wins.

mod cancel_for_suppression;
mod destroy_held_if_needed;
mod dispatch_outcome;
mod receipt_step;
mod step;
mod worker_lost;

pub(crate) mod tests_support;

pub(crate) use step::step;
