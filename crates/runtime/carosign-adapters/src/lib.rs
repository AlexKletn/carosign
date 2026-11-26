//! Adapters: Runtime ports connected to real performers.
//!
//! ⚠️ The rule for which this krate exists (document 02): Runtime ** doesn't import ** Linux,
//! Goodix, or storage. `ports`The root of the assembly decides who's behind them. Adapters
//! are the layer, and only they know both worlds.
//!
//! ⚠️ No adapter gives trusted values from the outside: `ValidatedGrant` It comes from the
//! source of politics. `ResourceLease` - from the middleman, `AssuranceAssessment` The
//! adapter just connects.

pub mod boot;
pub mod clock;
pub mod evidence;
pub mod gateway;
pub mod identity;
pub mod mutation;
pub mod policy;
pub mod projection;
pub mod provider;
pub mod recovery;
pub mod resource;

pub use boot::{BootError, BootReport, compose};
pub use clock::{StepClock, SystemClock};
pub use evidence::{EvidenceBridge, ProtectedAnswer};
pub use gateway::{Gateway, GatewayStep};
pub use identity::{PeerIdentity, subject_id, subject_number};
pub use mutation::{JournalMutationPort, StorageFailure};
pub use policy::{Action, FileIdentity, FilePolicy, Policy as TrustPolicy, Rule};
pub use projection::to_mutation_change;
pub use provider::{ProviderBridge, ProviderFailure};
pub use resource::{BrokerResourcePort, GrantOutcome, ResourceBridge};
