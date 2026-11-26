//! The performer of effects: the engine asks, they do it here, the actor returns observation.
//!
//! ⚠️ The rule for which this file exists is that **effects are executed in order, and
//! confirmation is returned to the actor, rather than ruling the state directly**.
//!
//! ⚠️ Shipping plans come from the outside: the set of authorized registrations and
//! authorization mode are known to the actor, not the performer. The performer does not
//! invent either a blank set or “somehow” authorization (Document 29, D-03; Document 31).

mod dispatch_plans;
mod effect_runner;
mod execution;

pub use dispatch_plans::DispatchPlans;
pub use effect_runner::EffectRunner;
pub use execution::Execution;
