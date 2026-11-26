//! Change machine (registration and deletion): preparation → permission → sending →
//! reconciliation → fixing.
//!
//! Table — document 04, “Mutation state machine”, supplemented by decisions B-01 and B-02
//! (see para. `docs/history/development-notes.md` and `docs/history/ZAMETKI.md`).
//!
//! ⚠️ Non-negotiable order:
//!
//! Persist the intent before hardware work. Persist `DispatchArmed` and its send queue entry
//! atomically before sending. Commit the registry and journal phase atomically. Finish only
//! when the scope is safe.
//!
//! ⚠️ In this module **no** `&mut OperationState`: he's just counting the next step.
//! `decide`.

mod mutation_input;
mod mutation_step;
mod mutation_step_outcome;
mod step;

pub use mutation_input::MutationInput;
pub use mutation_step::{MutationStep, effects_for, phase_for};
pub use mutation_step_outcome::MutationStepOutcome;
pub use step::step;
