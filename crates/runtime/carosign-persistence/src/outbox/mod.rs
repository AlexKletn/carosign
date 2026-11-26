//! Record the effects queue: stable address and state, not right to send.

use carosign_core::ids::{CommandId, MutationId};
use carosign_engine::transition::EffectKey;

mod effect_phase;
pub use effect_phase::EffectPhase;
mod effect_outbox_record;
pub use effect_outbox_record::EffectOutboxRecord;
