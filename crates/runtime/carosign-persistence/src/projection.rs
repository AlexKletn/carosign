//! Assembling a verified projection of the registration command (Document 34, B-14).
//!
//! ⚠️ The rule for which this file exists is that the difference between "new" and
//! "replacement" is solved by **plan transition**, not empty fields. The projection is built
//! from **unchangeable plan** and **one** registry reading under the same area lock, and the
//! old bundle is checked across all fields.
//!
//! ⚠️ There is no access to the vault and no repository: the collector receives the records
//! that were given to him. Who and with what blocking read them is the business of the
//! defiant, but the caller must be trusted (Runtime).
//!
//! The collector validates the projection before send authorization. It assigns no numbers,
//! writes no journal records, and sends no requests.

use core::fmt;

/// Storage type in projection.
pub use crate::mutation_plan::StoredBindingKind as BindingKind;
use carosign_core::enrollment::EnrollmentRef;
use carosign_core::ids::EnrollmentId;

use crate::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
use crate::mutation_plan::{MutationPlan, Scope, StoredBindingKind, Transition};

/// Builds a projection of the registration command.
///
/// ⚠️ The procedure for inspections is part of the contract (document 34, § 2):
///
/// 1. the transition plan decides what to build; 2. the old goal is searched **exactly**, not
/// the "first shot"; 3. the subject, authenticator, generation, view, format and digest
/// references are checked; 4. only then the projection is issued.
///
/// ⚠️ `sha256` comes from `mutation_plan`: one implementation for the entire assembly.
pub fn build(
    plan: &MutationPlan,
    scope: Scope,
    scope_entries: &[RegistryEntry],
) -> Result<Projection, ProjectionError> {
    plan.validate(scope)
        .map_err(|_| ProjectionError::PlanShape)?;
    check_scope(plan, scope_entries)?;
    match plan.transition {
        Transition::EnrollNew => {
            // ⚠️ Registration: There should be no old purpose.
            if plan.old_target.is_some() {
                return Err(ProjectionError::PlanShape);
            }
            let Some(new) = plan.new_enrollment else {
                return Err(ProjectionError::PlanShape);
            };
            Ok(Projection {
                change: ProjectionChange::Create {
                    planned_new: EnrollmentRef {
                        id: new,
                        authenticator: scope.authenticator,
                        generation: carosign_core::enrollment::EnrollmentGeneration::new(
                            plan.new_generation,
                        )
                        .map_err(|_| ProjectionError::PlanShape)?,
                    },
                },
                scope_size: scope_entries.len(),
            })
        }
        Transition::EnrollReplace => {
            let (old, binding) = checked_old(plan, scope_entries)?;
            let Some(new) = plan.new_enrollment else {
                return Err(ProjectionError::PlanShape);
            };
            Ok(Projection {
                change: ProjectionChange::Replace {
                    planned_new: EnrollmentRef {
                        id: new,
                        authenticator: old.authenticator,
                        generation: carosign_core::enrollment::EnrollmentGeneration::new(
                            plan.new_generation,
                        )
                        .map_err(|_| ProjectionError::PlanShape)?,
                    },
                    old: binding,
                },
                scope_size: scope_entries.len(),
            })
        }
        Transition::Remove => {
            // ⚠️ Deletion does not build a new registration; projection is only a verified
            // old bundle.
            let _ = checked_old(plan, scope_entries)?;
            Err(ProjectionError::PlanShape)
        }
    }
}

/// Verifies each area record, not just the old target selected.
pub(crate) fn check_scope(
    plan: &MutationPlan,
    entries: &[RegistryEntry],
) -> Result<(), ProjectionError> {
    if entries.len() != plan.pre_bindings.len() {
        return Err(ProjectionError::ScopeMismatch);
    }
    for snapshot in &plan.pre_bindings {
        let mut matches = entries
            .iter()
            .filter(|entry| entry.enrollment.id == snapshot.enrollment.id);
        let entry = matches.next().ok_or(ProjectionError::ScopeMismatch)?;
        if matches.next().is_some()
            || entry.enrollment != snapshot.enrollment
            || entry.subject != snapshot.subject
        {
            return Err(ProjectionError::ScopeMismatch);
        }
        if entry.status != EnrollmentStatus::Active {
            return Err(ProjectionError::NotActive);
        }
        if entry.kind != snapshot.kind {
            return Err(ProjectionError::KindMismatch);
        }
        if entry.format_id != snapshot.format_id || entry.format_version != snapshot.format_version
        {
            return Err(ProjectionError::FormatMismatch);
        }
        if crate::mutation_plan::sha256(&entry.provider_ref) != snapshot.provider_ref_digest {
            return Err(ProjectionError::ReferenceMismatch);
        }
    }
    Ok(())
}

/// Checks the old target on plan and registry.
fn checked_old(
    plan: &MutationPlan,
    scope_entries: &[RegistryEntry],
) -> Result<(EnrollmentRef, EnrollmentBinding), ProjectionError> {
    let old = plan.old_target.ok_or(ProjectionError::MissingOldTarget)?;

    // ⚠️ The old goal must be in the picture of the plan: otherwise there is nothing to check
    // with.
    let snapshot = plan
        .pre_bindings
        .iter()
        .find(|snapshot| snapshot.enrollment == old)
        .ok_or(ProjectionError::OldMissingOrStale { enrollment: old.id })?;

    // ⚠️ The image of the area must be the same size: an excess or missing entry means that
    // the registry has moved below us.
    if scope_entries.len() != plan.pre_bindings.len() {
        return Err(ProjectionError::ScopeMismatch);
    }

    let entry = scope_entries
        .iter()
        .find(|entry| entry.enrollment == old)
        .ok_or(ProjectionError::OldMissingOrStale { enrollment: old.id })?;

    if entry.subject != snapshot.subject || entry.enrollment.authenticator != old.authenticator {
        return Err(ProjectionError::OldOutOfScope);
    }
    if entry.status != EnrollmentStatus::Active {
        return Err(ProjectionError::NotActive);
    }
    if entry.kind != snapshot.kind {
        return Err(ProjectionError::KindMismatch);
    }
    if entry.format_id != snapshot.format_id || entry.format_version != snapshot.format_version {
        return Err(ProjectionError::FormatMismatch);
    }
    let digest = crate::mutation_plan::sha256(&entry.provider_ref);
    if digest != snapshot.provider_ref_digest {
        return Err(ProjectionError::ReferenceMismatch);
    }

    Ok((
        old,
        EnrollmentBinding {
            enrollment: old,
            kind: entry.kind,
            provider_ref: entry.provider_ref.clone(),
            format_id: if entry.format_id.is_empty() {
                None
            } else {
                Some(entry.format_id.clone())
            },
            format_version: entry.format_version,
        },
    ))
}

#[path = "projection.tests.rs"]
#[cfg(test)]
mod tests;

mod enrollment_binding;
pub use enrollment_binding::EnrollmentBinding;
mod projection_change;
pub use projection_change::ProjectionChange;
mod registry_entry;
pub use registry_entry::RegistryEntry;
mod projection;
pub use projection::Projection;
mod projection_error;
pub use projection_error::ProjectionError;
