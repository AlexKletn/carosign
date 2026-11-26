use super::BindingSnapshot;
use super::PlanError;
use super::PlanTransition;
use crate::identity::SubjectRef;
use crate::ids::AuthenticatorId;

/// A shape-checked immutable shot of the full plan.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MutationPlan {
    subject: SubjectRef,
    authenticator: AuthenticatorId,
    transition: PlanTransition,
    pre_bindings: Vec<BindingSnapshot>,
    scope_digest: [u8; 32],
    label: Option<crate::enrollment::EnrollmentLabel>,
}

impl MutationPlan {
    /// Checks the shape and saves the picture without sorting and normalization.
    ///
    /// Checking the digest and completeness of the registry is mandatory at the trusted
    /// border; this designer does not confirm them by itself.
    pub fn new(
        subject: SubjectRef,
        authenticator: AuthenticatorId,
        transition: PlanTransition,
        pre_bindings: Vec<BindingSnapshot>,
        scope_digest: [u8; 32],
    ) -> Result<Self, PlanError> {
        if pre_bindings.len() > 256 {
            return Err(PlanError::TooManyBindings);
        }
        for entry in &pre_bindings {
            if entry.subject != subject || entry.enrollment.authenticator != authenticator {
                return Err(PlanError::OutOfScope);
            }
            if entry.format_id.len() > 128 || entry.format_id.chars().any(char::is_control) {
                return Err(PlanError::InvalidFormat);
            }
        }
        if pre_bindings.windows(2).any(|pair| {
            pair[0].enrollment.id == pair[1].enrollment.id
                || (pair[0].enrollment.id, pair[0].enrollment.generation)
                    >= (pair[1].enrollment.id, pair[1].enrollment.generation)
        }) {
            return Err(PlanError::NonCanonicalBindings);
        }
        let (old, new) = match transition {
            PlanTransition::Create { planned_new } => (None, Some(planned_new)),
            PlanTransition::Replace { old, planned_new } => (Some(old), Some(planned_new)),
            PlanTransition::Remove { old } => (Some(old), None),
        };
        if old.is_some_and(|value| value.authenticator != authenticator)
            || new.is_some_and(|value| value.authenticator != authenticator)
        {
            return Err(PlanError::OutOfScope);
        }
        if old.is_some_and(|value| !pre_bindings.iter().any(|entry| entry.enrollment == value))
            || new.is_some_and(|value| {
                pre_bindings
                    .iter()
                    .any(|entry| entry.enrollment.id == value.id)
            })
        {
            return Err(PlanError::InvalidTransition);
        }
        Ok(Self {
            subject,
            authenticator,
            transition,
            pre_bindings,
            scope_digest,
            label: None,
        })
    }

    /// The subject of the plan.
    pub fn subject(&self) -> SubjectRef {
        self.subject
    }
    /// Plan apparatus.
    pub fn authenticator(&self) -> AuthenticatorId {
        self.authenticator
    }
    /// Planned crossing.
    pub fn transition(&self) -> PlanTransition {
        self.transition
    }
    /// The initial image of the area.
    pub fn pre_bindings(&self) -> &[BindingSnapshot] {
        &self.pre_bindings
    }
    /// The amount of consistency verified by storage.
    pub fn scope_digest(&self) -> &[u8; 32] {
        &self.scope_digest
    }
    /// The tag is included in a sustainable registration plan; no removal is required.
    pub fn with_label(
        mut self,
        label: Option<crate::enrollment::EnrollmentLabel>,
    ) -> Result<Self, PlanError> {
        if label.is_some() && matches!(self.transition, PlanTransition::Remove { .. }) {
            return Err(PlanError::InvalidTransition);
        }
        self.label = label;
        Ok(self)
    }
    /// A proven plan label, if it has been retained.
    pub fn label(&self) -> Option<&crate::enrollment::EnrollmentLabel> {
        self.label.as_ref()
    }
}
