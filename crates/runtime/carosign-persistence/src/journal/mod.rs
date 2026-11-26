//! Modification log: `prepare → arm → observe → commit → finish`, plus a reconciliation.
//!
//! ⚠️ Non-negotiable procedure (document 10):
//!
//! Persist the intent before hardware work. Persist `DispatchArmed` and its send queue entry
//! atomically before sending. Commit the registry and journal phase atomically. Finish only
//! when the scope is safe.
//!
//! ⚠️ Ticket [`ArmedMutationTicket`] issued ** after the confirmed record.
//! Until it is issued, there is simply no place to send a changing command: the caller does
//! not have what it requires.`crate::journal::MutationRepository`].
//!
//! ⚠️ Tickets cannot be invented from the magazine when restarting: protected from `Clone`
//! And he doesn't have a designer, just `arm` He's making it.

mod arm_mutation;
mod armed_mutation_ticket;
mod commit_enrollment;
mod commit_reconciliation;
mod decode_record;
mod mutation_observation;
mod mutation_repository;
mod prepare_mutation;
mod prepared_mutation;
mod registry_commit;

pub use arm_mutation::ArmMutation;
pub use armed_mutation_ticket::ArmedMutationTicket;
pub use commit_enrollment::CommitEnrollment;
pub use commit_reconciliation::CommitReconciliation;
pub use decode_record::decode_record;
pub use mutation_observation::MutationObservation;
pub use mutation_repository::MutationRepository;
pub use prepare_mutation::PrepareMutation;
pub use prepare_mutation::validate_prepare;
pub use prepared_mutation::PreparedMutation;
pub use registry_commit::RegistryCommit;
