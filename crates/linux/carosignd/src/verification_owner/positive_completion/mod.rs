use crate::effect_owner::EffectOwner;
use crate::effect_owner::EffectOwnerError;
use crate::operations::OperationRegistry;
use carosign_core::ids::OperationId;
use carosign_runtime::operation::actor::Step;

mod positive_completion;
pub(crate) use positive_completion::PositiveCompletion;
mod positive_completion_stage;
use positive_completion_stage::PositiveCompletionStage;
mod positive_completion_error;
pub(crate) use positive_completion_error::PositiveCompletionError;
