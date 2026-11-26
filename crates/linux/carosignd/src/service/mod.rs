//! Service as a whole: ports, operations, steps to the end.
//!
//! ⚠️ Rule 05: The service makes decisions -- it collects ports, asks for authority, creates
//! an actor and drives steps -- the engine, the actor and the ports decide everything else.
//!
//! ⚠️ Ports come **outside **`assemble`), not created here: so the service remains thin, and
//! the stand in checks - possible.
//!
//! A refusal must leave no pending operation. If admission failed, the operation was never
//! created.

mod caller_bound_authority;
mod client_preparation_error;
mod prepared_client_verification;
mod served;
mod service;

pub use client_preparation_error::ClientPreparationError;
pub use prepared_client_verification::PreparedClientVerification;
pub use served::Served;
pub use service::Service;

pub(crate) use service::client_verification_intent;
