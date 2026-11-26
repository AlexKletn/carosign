//! A magazine is like a runtime port.
//!
//! ⚠️ The rule for which this adapter exists is: **Resilience is a report.** The port
//! returns. `durable: true` An unconfirmed record is a failure, and the altering command is
//! not sent at that time (B-02).
//!
//! ⚠️ The adapter decides what to write: the steps of the log are given by the domain. It
//! only translates the "stage of operation" into a storage call.

mod authorized_completed_outcome;
mod authorized_mutation_replay;
mod completed_mutation_reader;
mod journal_mutation_port;
mod mutation_intent;
mod outcome_read_error;
mod outcome_replay_error;
mod removal_recovery;
mod removal_recovery_error;
mod storage_failure;

pub use authorized_completed_outcome::authorized_completed_outcome;
pub use authorized_completed_outcome::authorized_completed_outcome_by_operation;
pub use authorized_mutation_replay::AuthorizedMutationReplay;
pub use authorized_mutation_replay::authorized_completed_replay;
pub use completed_mutation_reader::CompletedMutationReader;
pub use journal_mutation_port::JournalMutationPort;
pub use mutation_intent::MutationIntent;
pub use outcome_read_error::OutcomeReadError;
pub use outcome_replay_error::OutcomeReplayError;
pub use removal_recovery::restore_removal_for_reconciliation;
pub use removal_recovery::restore_removal_for_reconciliation_v4;
pub use removal_recovery_error::RemovalRecoveryError;
pub use storage_failure::StorageFailure;
