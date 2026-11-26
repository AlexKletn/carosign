//! Limited transmission of actor steps and effects to individual performers.
//!
//! It is an internal owner in memory: it does not replace a stable log, authority check, or
//! hardware cleaning confirmation.

mod batch;
mod drive_error;
mod drive_outcome;
mod effect_claim;
mod effect_owner_error;
mod effect_slot;
mod effect_state;
mod evidence_evaluation;
mod evidence_evaluation_slot;
mod owner;
mod record_claim;
mod record_result;
mod record_slot;
mod step_publish_error;

pub(crate) use drive_error::DriveError;
pub(crate) use drive_outcome::DriveOutcome;
pub(crate) use effect_claim::EffectClaim;
pub(crate) use effect_owner_error::EffectOwnerError;
pub(crate) use owner::EffectOwner;
pub(crate) use record_claim::RecordClaim;
pub(crate) use record_result::RecordResult;
pub(crate) use step_publish_error::StepPublishError;
