//! Typed Domain IDs.
//!
//! ⚠️ Every day. `*Id` This is a ** separate type with a length of exactly 16 bytes**, and
//! not a pseudonym for the general one. `Uuid`A pseudonym would allow you to substitute the
//! transaction ID where the registration ID is waiting, and the compiler would be silent.
//!
//! Rules (document 03):
//!
//! * Zero is rejected.`InvalidId`];
//! * Byte order canonical, `as_bytes` He gives exactly what is in his memory.
//! * The default designer is closed: [`IdSource`] - ** trait only, his
//! The incarnation is created by the assembly (composition root) and lives in time.
//! * none `Uuid::new_v4` inside `core` and `engine` - generation is coming
//! only through`IdSource`].
//!
//! No type here does I/O and doesn't know about the system.

mod authenticator_id;
mod authority_id;
mod boot_id;
mod challenge_ref;
mod command_id;
mod connection_id;
mod device_id;
mod effect_id;
mod enrollment_id;
mod evidence_id;
mod grant_id;
mod id_length;
mod id_source;
mod id_unavailable;
mod idempotency_key;
mod invalid_id;
mod lease_id;
mod mutation_id;
mod opaque_id;
mod operation_id;
mod package_id;
mod profile_id;
mod provider_id;
mod recipient_ref;
mod resource_id;
mod subject_id;
mod worker_instance_id;

pub use authenticator_id::AuthenticatorId;
pub use authority_id::AuthorityId;
pub use boot_id::BootId;
pub use challenge_ref::ChallengeRef;
pub use command_id::CommandId;
pub use connection_id::ConnectionId;
pub use device_id::DeviceId;
pub use effect_id::EffectId;
pub use enrollment_id::EnrollmentId;
pub use evidence_id::EvidenceId;
pub use grant_id::GrantId;
pub use id_length::ID_LEN;
pub use id_source::IdSource;
pub use id_unavailable::IdUnavailable;
pub use idempotency_key::IdempotencyKey;
pub use invalid_id::InvalidId;
pub use lease_id::LeaseId;
pub use mutation_id::MutationId;
pub use operation_id::OperationId;
pub use package_id::PackageId;
pub use profile_id::ProfileId;
pub use provider_id::ProviderId;
pub use recipient_ref::RecipientRef;
pub use resource_id::ResourceId;
pub use subject_id::SubjectId;
pub use worker_instance_id::WorkerInstanceId;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
