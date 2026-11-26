//! Confirmation of effects and observations that the engine takes in.
//!
//! ⚠️ The engine only asks for the effects. It performs their time and returns the
//! observation. It doesn't make up the confirmation: no observation, no transition.
//!
//! ⚠️ `SendFailedAfterArm` - not "not sent", but "unknown", after which the changing command
//! is not repeated (C-04/E-04/E-05).

mod command_mark;
mod domain_input;
mod effect_receipt;
mod effect_spec;
mod planned_effect;

pub use command_mark::CommandMark;
pub use domain_input::DomainInput;
pub use effect_receipt::EffectReceipt;
pub use effect_spec::EffectSpec;
pub use planned_effect::PlannedEffect;
