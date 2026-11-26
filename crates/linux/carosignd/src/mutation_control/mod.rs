//! Customer review and interruption for one altering operation.
//!
//! After a steady arm, stopping the process does not prove that there is no change. If the
//! outcome is unknown, the area remains closed for reconciliation.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use carosign_core::ids::{MutationId, OperationId};
use carosign_persistence::store::MetadataStore;
use carosign_runtime::ports::TimeSource;

use crate::mutation_dispatch::PreparedMutation;

mod mutation_control;
pub(crate) use mutation_control::MutationControl;
mod control_error;
pub(crate) use control_error::ControlError;
