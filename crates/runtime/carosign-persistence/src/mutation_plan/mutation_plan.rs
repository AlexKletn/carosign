use core::fmt;

use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::EnrollmentId;

use super::{BindingSnapshot, PlanError, Scope, StoredBindingKind, Transition, canonical_digest};

/// The limit of the shots in the plan.
pub const SNAPSHOTS_MAX: usize = 256;

/// An immutable plan of change.
#[derive(Clone, PartialEq, Eq)]
pub struct MutationPlan {
    /// What's planned.
    pub transition: Transition,
    /// New registration number (required for registration, empty for deletion).
    pub new_enrollment: Option<EnrollmentId>,
    /// New generation (required for registration).
    pub new_generation: u64,
    /// Old purpose: mandatory for replacement and deletion, **not available** for new
    /// (document 34).
    pub old_target: Option<EnrollmentRef>,
    /// Pictures of the entire affected area, ** sorted by `(enrollment_id, generation)`.
    pub pre_bindings: Vec<BindingSnapshot>,
    /// Registration tag in v4, included in the plan digest.
    pub requested_label: Option<String>,
    /// Digest of the canonical recording of the area.
    pub scope_digest: [u8; 32],
}

impl fmt::Debug for MutationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MutationPlan")
            .field("transition", &self.transition)
            .field("snapshots", &self.pre_bindings.len())
            .finish_non_exhaustive()
    }
}

impl MutationPlan {
    /// Creates an immutable domain image after checking v3 and digest.
    ///
    /// This is a check of consistency of metadata, not proof of hardware or trusted origin of the
    /// original image.
    pub fn to_core(
        &self,
        scope: Scope,
    ) -> Result<carosign_core::operation::plan::MutationPlan, PlanError> {
        use carosign_core::operation::plan::{
            BindingKind, BindingSnapshot as CoreSnapshot, MutationPlan as CorePlan, PlanTransition,
        };
        self.validate(scope)?;
        let planned_new = self
            .new_enrollment
            .map(|id| {
                Ok(EnrollmentRef {
                    id,
                    authenticator: scope.authenticator,
                    generation: EnrollmentGeneration::new(self.new_generation)
                        .map_err(|_| PlanError::GenerationShape)?,
                })
            })
            .transpose()?;
        let transition = match (self.transition, self.old_target, planned_new) {
            (Transition::EnrollNew, None, Some(planned_new)) => {
                PlanTransition::Create { planned_new }
            }
            (Transition::EnrollReplace, Some(old), Some(planned_new)) => {
                PlanTransition::Replace { old, planned_new }
            }
            (Transition::Remove, Some(old), None) => PlanTransition::Remove { old },
            _ => return Err(PlanError::InvalidSemanticPlan),
        };
        let entries = self
            .pre_bindings
            .iter()
            .map(|entry| CoreSnapshot {
                enrollment: entry.enrollment,
                subject: entry.subject,
                kind: match entry.kind {
                    StoredBindingKind::ProviderOpaque => BindingKind::ProviderOpaque,
                    StoredBindingKind::ScopedVault => BindingKind::ScopedVault,
                },
                provider_ref_digest: entry.provider_ref_digest,
                format_id: entry.format_id.clone(),
                format_version: entry.format_version,
            })
            .collect();
        CorePlan::new(
            scope.subject,
            scope.authenticator,
            transition,
            entries,
            self.scope_digest,
        )
        .and_then(|plan| {
            plan.with_label(
                self.requested_label
                    .as_deref()
                    .map(carosign_core::enrollment::EnrollmentLabel::new)
                    .transpose()
                    .map_err(|_| carosign_core::operation::plan::PlanError::InvalidFormat)?,
            )
        })
        .map_err(|_| PlanError::InvalidSemanticPlan)
    }

    /// Checks the plan and **checks the digest** with the canonical record.
    ///
    /// ⚠️ Order: shape (transition, generation, number), then sorting, then area, then
    /// digest. Digest last - otherwise it would count for garbage.
    pub fn validate(&self, scope: Scope) -> Result<(), PlanError> {
        // 1. Form of transition.
        match (self.transition, self.new_enrollment) {
            (Transition::EnrollNew | Transition::EnrollReplace, Some(id)) => {
                if id.as_bytes().iter().all(|byte| *byte == 0) {
                    return Err(PlanError::TransitionShape);
                }
            }
            (Transition::Remove, None) => {}
            _ => return Err(PlanError::TransitionShape),
        }
        // 2. Generation.
        if self.transition.creates_enrollment() {
            if self.new_generation == 0 {
                return Err(PlanError::GenerationShape);
            }
        } else if self.new_generation != 0 {
            return Err(PlanError::GenerationShape);
        }
        // Old purpose: mandatory for replacement and removal, no new purpose.
        match (self.transition, self.old_target) {
            (Transition::EnrollNew, None) => {}
            (Transition::EnrollNew, Some(_)) => return Err(PlanError::OldTargetShape),
            (Transition::EnrollReplace | Transition::Remove, Some(old)) => {
                if old.authenticator != scope.authenticator {
                    return Err(PlanError::OutOfScope);
                }
                // ⚠️ A new registration must be a different one: a replacement with the same
                // number is not a replacement.
                if let Some(new) = self.new_enrollment
                    && new == old.id
                {
                    return Err(PlanError::SameEnrollment);
                }
            }
            (Transition::EnrollReplace | Transition::Remove, None) => {
                return Err(PlanError::OldTargetShape);
            }
        }
        // Number of pictures.
        if self.pre_bindings.len() > SNAPSHOTS_MAX {
            return Err(PlanError::TooMany {
                actual: self.pre_bindings.len(),
            });
        }
        if let Some(new) = self.new_enrollment
            && self
                .pre_bindings
                .iter()
                .any(|snapshot| snapshot.enrollment.id == new)
        {
            return Err(PlanError::SameEnrollment);
        }
        // Sorting and uniqueness.
        for pair in self.pre_bindings.windows(2) {
            let left = (pair[0].enrollment.id, pair[0].enrollment.generation);
            let right = (pair[1].enrollment.id, pair[1].enrollment.generation);
            // Register indexed by EnrollmentId: two versions of the same registration may not
            // simultaneously make up the original image.
            if left.0 == right.0 || left >= right {
                return Err(PlanError::Unsorted);
            }
        }
        // In order to replace and delete the old target must be in the pictures.
        if let Some(old) = self.old_target
            && !self
                .pre_bindings
                .iter()
                .any(|snapshot| snapshot.enrollment == old)
        {
            return Err(PlanError::OldTargetShape);
        }
        // 5. Area.
        for snapshot in &self.pre_bindings {
            if snapshot.subject != scope.subject
                || snapshot.enrollment.authenticator != scope.authenticator
            {
                return Err(PlanError::OutOfScope);
            }
            if snapshot.format_id.len() > 128 {
                return Err(PlanError::FormatTooLong);
            }
            if snapshot.format_id.chars().any(char::is_control) {
                return Err(PlanError::BadFormat);
            }
        }
        // Digest 6.
        let expected = canonical_digest(scope, self)?;
        if expected != self.scope_digest {
            return Err(PlanError::DigestMismatch);
        }
        Ok(())
    }

    /// He compiles a plan, counting the digest himself.
    ///
    /// ⚠️ Sort the pictures can only call: order is part of the canonical record, not
    /// decoration.
    pub fn build(
        transition: Transition,
        old_target: Option<EnrollmentRef>,
        new_enrollment: Option<EnrollmentId>,
        new_generation: u64,
        pre_bindings: Vec<BindingSnapshot>,
        scope: Scope,
    ) -> Result<Self, PlanError> {
        let mut plan = Self {
            transition,
            old_target,
            new_enrollment,
            new_generation,
            pre_bindings,
            requested_label: None,
            scope_digest: [0u8; 32],
        };
        plan.scope_digest = canonical_digest(scope, &plan)?;
        plan.validate(scope)?;
        Ok(plan)
    }

    /// Includes an accurate label in an immutable v4 registration plan.
    pub fn with_requested_label(mut self, scope: Scope, label: &str) -> Result<Self, PlanError> {
        if self.transition == Transition::Remove || label.len() > 64 {
            return Err(PlanError::TransitionShape);
        }
        self.requested_label = Some(label.to_owned());
        self.scope_digest = canonical_digest(scope, &self)?;
        self.validate(scope)?;
        Ok(self)
    }
}
