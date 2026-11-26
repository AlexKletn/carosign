//! Worker Context: What the platform has delivered and what the worker knows.
//!
//! ⚠️ There is not a single argument of “I am a provider”: the worker does not confirm
//! himself. `WorkerInstanceId`, `PackageId` and `ProfileId` come in `Initialize`eh
//! `AuthenticatorId` in `BindAuthenticator`.
//! Everything in this context is out there.

mod context_error;
mod worker_context;

pub use context_error::ContextError;
pub use worker_context::WorkerContext;
