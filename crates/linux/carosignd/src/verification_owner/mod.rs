//! Transferring the steps of the test session to the owner of the effects without loss on
//! failure.
//!
//! Trust in the worker, management of the duration of the operation and delivery to the
//! customer remain the responsibility of the staff; this owner does not issue authority.

mod matched_evidence_owner;
mod positive_completion;
mod verification_flow;
mod verification_owner;
mod verification_start;

#[cfg(test)]
pub(crate) use matched_evidence_owner::MatchedEvidenceOwner;
#[cfg(test)]
pub(crate) use positive_completion::PositiveCompletion;
pub(crate) use verification_flow::{
    FlowError, FlowProgress, VerificationFlow, VerificationFlowTarget,
};
pub(crate) use verification_owner::AdvanceOutcome;
#[cfg(test)]
pub(crate) use verification_owner::{OwnerError, OwnerEvent, VerificationOwner};
#[cfg(test)]
pub(crate) use verification_start::{StartError, VerificationStart, VerificationStartPorts};
