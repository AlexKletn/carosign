//! The vault's limits.
//!
//! ⚠️ These are **schema numbers, not time settings, and are only changed by migrating the
//! schema with the log (document 10, "Migration").

/// Length of nested value `StoredValue`.
///
/// Document 23: for metadata from 1 to 65,536 bytes, for templates - up to 8 MiB.
pub const STORED_VALUE_MAX: usize = 65_536;

/// The length of the nested value of the template.
pub const TEMPLATE_VALUE_MAX: usize = 8 * 1024 * 1024;

/// The key is 16 bytes long; it is the length of any identifier.
pub const ID_KEY_LEN: usize = 16;

/// Length of the registration index key: `AuthorityId || SubjectId || AuthenticatorId ||
/// EnrollmentId`.
pub const ENROLLMENT_INDEX_KEY_LEN: usize = 64;

/// Length of the area lock key: `AuthorityId || SubjectId || AuthenticatorId`.
pub const SCOPE_LOCK_KEY_LEN: usize = 48;

/// Length of the effect queue key: `OperationId || transition(u64 BE) || position(u32 BE)`.
pub const OUTBOX_KEY_LEN: usize = 16 + 8 + 4;

/// Template key length: `EnrollmentId || generation(u64 BE)`.
pub const TEMPLATE_KEY_LEN: usize = 24;

/// Length of the idempotence key: `caller_binding_digest(32) || key(16)`.
pub const IDEMPOTENCY_KEY_LEN: usize = 32 + 16;

/// The length of the hash in the records (SHA-256).
pub const DIGEST_LEN: usize = 32;

/// The length of the registration tag, in UTF-8 bytes.
pub const LABEL_MAX: usize = 64;

/// The length of the opaque link of the provider.
pub const OPAQUE_REF_MAX: usize = 4_096;

/// The length of the checksum recording (in the combat scheme - SHA-256).
pub const RECORD_DIGEST_LEN: usize = 64;

/// Length of the subject's canonical record.
pub const SUBJECT_CANONICAL_LEN: usize = 32;
