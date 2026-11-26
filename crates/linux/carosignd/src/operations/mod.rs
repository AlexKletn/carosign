//! Limited ownership of operations between service steps.
//!
//! The table does not replace the source of authority, the observation of the connection, or
//! the restoration of the broker; it saves the actor when the call is interrupted.

mod admission_failure;
mod cleanup_failure;
mod cleanup_progress;
mod cleanup_stage;
mod connection_usage;
mod deferred_control_failure;
mod entry;
mod idempotent_admission;
mod limits;
mod mutation_binding_failure;
mod observation_failure;
mod observation_progress;
mod operation_guard;
mod operation_registry;
mod owned_operation;
mod owner_delivery;
mod owner_delivery_failure;
mod owner_send_error;
mod owner_send_progress;
mod positive_delivery;
mod prepared_owner_verification;
mod retired_operation;
pub(crate) use retired_operation::VERIFICATION_KEYS_MAX;
mod registry_entries;
mod reserved_positive_send;
mod retained_owner_verification;
mod scheduled_cleanup_progress;
mod terminal_record_failure;
mod terminal_record_stage;
mod trusted_caller_binding;
mod verification_binding_failure;
mod verification_binding_stage;
mod verification_dispatch_stage;
mod verification_preparation_failure;
mod verification_send;

pub use admission_failure::AdmissionFailure;
pub use cleanup_failure::CleanupFailure;
pub use cleanup_progress::CleanupProgress;
use cleanup_stage::CleanupStage;
pub(crate) use connection_usage::ConnectionHandle;
use connection_usage::ConnectionUsage;
pub(crate) use deferred_control_failure::DeferredControlFailure;
use entry::Entry;
pub(crate) use idempotent_admission::IdempotentAdmission;
pub(crate) use limits::DEFAULT_CONNECTIONS;
pub use limits::{OPERATIONS_MAX, OPERATIONS_PER_OWNER};
pub(crate) use mutation_binding_failure::MutationBindingFailure;
pub use observation_failure::ObservationFailure;
pub use observation_progress::ObservationProgress;
pub(crate) use operation_guard::OperationGuard;
pub use operation_registry::OperationRegistry;
pub(crate) use owned_operation::OwnedOperation;
pub(crate) use owner_delivery::OwnerVerificationDelivery;
pub use owner_delivery_failure::OwnerDeliveryFailure;
pub(crate) use owner_send_error::OwnerSendError;
pub(crate) use owner_send_progress::OwnerSendProgress;
pub(crate) use positive_delivery::PositiveDeliveryOwner;
pub use prepared_owner_verification::PreparedOwnerVerification;
use registry_entries::RegistryEntries;
use reserved_positive_send::send_reserved_positive;
use retained_owner_verification::RetainedOwnerVerification;
pub use scheduled_cleanup_progress::ScheduledCleanupProgress;
pub(crate) use terminal_record_failure::TerminalRecordFailure;
use terminal_record_stage::TerminalRecordStage;
pub(crate) use trusted_caller_binding::TrustedCallerBinding;
pub(crate) use verification_binding_failure::VerificationBindingFailure;
use verification_binding_stage::VerificationBindingStage;
use verification_dispatch_stage::VerificationDispatchStage;
pub use verification_preparation_failure::VerificationPreparationFailure;
pub(crate) use verification_send::VerificationSend;

mod live_operation_status;
pub(crate) use live_operation_status::LiveOperationStatus;

mod operation_cancellation;
