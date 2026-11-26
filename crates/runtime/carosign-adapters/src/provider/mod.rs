//! A wire to the provider as a runtime port.
//!
//! ⚠️ The rule for which this adapter exists (Document 02): Runtime ** does not import any
//! transport or Goodix.`ProviderExecutionPort`]; the adapter translates the command into a
//! wire frame.
//!
//! ⚠️ Document 29 (D-03): Checking **always** carries a set of authorized registrations.
//! `PermittedSet` It does not allow, and the provider without a set is not called.
//!
//! ⚠️ Document 31: Registration is either nothing (regime) `NONE`), or a link to the
//! authorization and digest of the call. ** The token itself** does not go through the
//! provider's wire.

mod binding_encoding;
mod mutate_plan;
mod mutation_frame;
mod mutation_frame_only;
mod provider_bridge;
mod provider_channel;
mod provider_failure;
mod verification_frame_context;

pub use mutate_plan::mutate_plan;
pub use mutation_frame::continuation_frame;
pub use mutation_frame::mutation_frame;
use mutation_frame_only::MutationFrameOnly;
pub use provider_bridge::ProviderBridge;
pub use provider_channel::ProviderChannel;
pub use provider_failure::ProviderFailure;
pub use verification_frame_context::VerificationFrameContext;
pub use verification_frame_context::verification_frame;
