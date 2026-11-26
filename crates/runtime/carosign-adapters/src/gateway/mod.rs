//! Actor's Gateway: step, performance of effects, return of observation.
//!
//! ⚠️ It's the only place where things change as a result of working in the outside world.
//!
//! 1. the actor takes a step and asks for effects; 2. the effects are executed in order; 3.
//! the observation returns to the actor — the state is ruled by him, not the performer; 4. if
//! there is no observation, the next step awaits.
//!
//! ⚠️ Modifying sending is not repeated: re-entry with the same `SendMutatingCommand` -
//! denial.

mod effect_gateway;
mod gateway_step;
mod observed_gateway_step;

pub use effect_gateway::Gateway;
pub use gateway_step::GatewayStep;
pub use observed_gateway_step::ObservedGatewayStep;
