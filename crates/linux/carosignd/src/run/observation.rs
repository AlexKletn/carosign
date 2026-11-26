use carosign_engine::{EffectReceipt, EffectSpec, PlannedEffect};

/// Observation of the service by an effect that has no execution on this layer.
///
/// ⚠️ Leasing gives the middleman; surveillance returns the service.
/// `RevalidateGrant` It is always implemented through the policy source below.
pub type Observation<'a> = dyn FnMut(PlannedEffect, EffectSpec) -> Option<EffectReceipt> + 'a;
