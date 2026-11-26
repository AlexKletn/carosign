//! Recording model: what is in the tables, and what should be.
//!
//! ⚠️ Records are domain values, not storage structures. How they are converted into bytes is
//! a matter of ...`redb::codec`](crate::redb::codec)This file only knows the meaning and
//! rules of form.

mod authenticator_record;
mod enrollment_status;
mod idempotency_record;
mod mutation_closure;
mod mutation_phase;
mod mutation_record;
mod mutation_terminal_outcome;
mod package_binding;
mod record_error;
mod storage_error;

pub use authenticator_record::AuthenticatorRecord;
pub use enrollment_status::EnrollmentStatus;
pub use idempotency_record::IdempotencyRecord;
pub use mutation_closure::MutationClosure;
pub use mutation_phase::MutationPhase;
pub use mutation_record::MutationRecord;
pub use mutation_terminal_outcome::MutationTerminalOutcome;
pub use package_binding::PackageBinding;
pub use record_error::RecordError;
pub use storage_error::StorageError;
