//! The state of the operation: common to all and its own in each species.
//!
//! ⚠️ The type of operation is **option**, not a field-character (decision C-03). The
//! verification cannot physically carry the state of change, and the registration cannot
//! represent the verification: the compiler will not give a separate feature. `kind !=
//! mutation.is_some()` It does not exist at all, so there is nothing to differ from it.
//!
//! [`OperationKind`](carosign_core::modality::OperationKind) from `core` It remains a
//! convenient representation of the species, but ** is derived from the body of the operation
//! ([]`OperationState::kind`] and is not stored in a second copy.

mod transition_index;
mod effect_position;
mod effect_key;
mod operation_phase;
mod worker_ref;
mod command_ref;
mod terminal_outcome;
mod common_operation_state;
mod verification_state;
mod mutation_state;
mod operation_body;
mod operation_state;

pub use transition_index::{TransitionIndex, TransitionIndexOverflow};
pub use effect_position::EffectPosition;
pub use effect_key::EffectKey;
pub use operation_phase::{OperationPhase, PhaseFamily};
pub use worker_ref::WorkerRef;
pub use command_ref::CommandRef;
pub use terminal_outcome::TerminalOutcome;
pub use common_operation_state::CommonOperationState;
pub use verification_state::{VerificationDelivery, VerificationState};
pub use mutation_state::{LiveMutationConfirmation, MutationState};
pub use operation_body::OperationBody;
pub use operation_state::OperationState;
