//! Operation cycle: from the client’s request to the end – in steps, without magic.
//!
//! ⚠️ Rule 05/22: The state of an operation is changed by the * * * actor. * There's a
//! sequence of steps around it: clearance, resources, sending, observing, outcome. * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//! * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
//!
//! Authority issuance and evidence verification are provided elsewhere.
//! The ports give them; the cycle only asks and receives an answer. Without the port, the
//! cycle does not assemble, and rightly so.
//!
//! ⚠️ The check is carried out ** on the set** of authorized registrations (document 29):
//! there is no empty set, and without a set, the step does not begin.

mod cycle_driver;
mod cycle_error;
mod cycle_step;
mod mutation_cycle;
mod stage;
mod verification_cycle;

pub use cycle_driver::operation_of;
pub use cycle_driver::step_actor;
pub use cycle_error::CycleError;
pub use cycle_step::CycleStep;
pub use mutation_cycle::MutationCycle;
pub use stage::Stage;
pub use verification_cycle::VerificationCycle;
