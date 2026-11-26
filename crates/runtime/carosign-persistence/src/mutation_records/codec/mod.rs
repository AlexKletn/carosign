use super::carosign::{common::v1 as common, storage::v1 as wire};
use super::record_error::corrupt;
use crate::model::{
    IdempotencyRecord, MutationPhase, MutationRecord, MutationTerminalOutcome, StorageError,
};
use crate::mutation_plan::{
    BindingSnapshot, MutationPlan, Scope, StoredBindingKind, Transition, sha256,
};
use crate::outbox::{EffectOutboxRecord, EffectPhase};
use crate::store::WriteTransaction;
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, CommandId, EnrollmentId, IdempotencyKey, MutationId, OperationId,
    SubjectId, WorkerInstanceId,
};
use prost::Message;

mod idempotency_codec;
pub use idempotency_codec::decode_idempotency;
pub use idempotency_codec::encode_idempotency;
mod outbox_codec;
pub use outbox_codec::decode_outbox;
pub use outbox_codec::encode_outbox;
pub use outbox_codec::validate_mutation_outbox;
mod mutation_codec;
pub use mutation_codec::decode_mutation;
pub use mutation_codec::encode_mutation;
pub(super) use mutation_codec::encode_record_for_schema;
use mutation_codec::validate_v3;
