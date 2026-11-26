//! Translation of the storage projection to the runtime port change.
//!
//! ⚠️ Rule of Document 02 (DAG): `carosign-persistence` It doesn't know about runtime, and
//! runtime doesn't know about storage. It's here in the adapter, and it's the only place that
//! the two worlds come into contact.
//!
//! ⚠️ Translation does not check anything again: checks are made `persistence::projection`
//! (plan + registry under lock area) There's only a change of type names, no smart additives.

use carosign_persistence::projection::{Projection, ProjectionChange};
use carosign_runtime::operation::bindings::{BindingKind, EnrollmentBinding};
use carosign_runtime::ports::MutationChange;

/// Translates the verified projection into a change.
///
/// ⚠️ The transition decides the option, not the empty fields: `Create` without the old
/// ligament, `Replace` with her (document 34).
pub fn to_mutation_change(projection: Projection) -> MutationChange {
    match projection.change {
        ProjectionChange::Create { planned_new } => MutationChange::Create { planned_new },
        ProjectionChange::Replace { planned_new, old } => MutationChange::Replace {
            planned_new,
            old: EnrollmentBinding {
                enrollment: old.enrollment,
                kind: match old.kind {
                    carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque => {
                        BindingKind::ProviderOpaque
                    }
                    carosign_persistence::mutation_plan::StoredBindingKind::ScopedVault => {
                        BindingKind::ScopedVault
                    }
                },
                provider_ref: old.provider_ref,
                format_id: old.format_id,
                format_version: old.format_version,
            },
        },
    }
}

#[path = "projection.tests.rs"]
#[cfg(test)]
mod tests;
